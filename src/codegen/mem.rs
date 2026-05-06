use std::fmt::Write as _;

use anyhow::{Result, anyhow, bail};
use llvm_ir::{
    Constant, Operand, Type, TypeRef,
    instruction::RMWBinOp,
    types::{FPType, Typed},
};

use super::names::{name_to_var, operand_str};
use super::types::{
    LayoutCx, align_up, is_aggregate, is_undef_or_zero, mem_bits, resolve_named, sign_extend,
};

pub(super) fn emit_load_at(
    out: &mut String,
    indent: &str,
    dest: &str,
    addr: &str,
    ty: &TypeRef,
) -> Result<()> {
    let expr = match ty.as_ref() {
        Type::FPType(FPType::Single) => format!("_load_f32({addr})"),
        Type::FPType(FPType::Double) => format!("_load_f64({addr})"),
        _ => {
            let bits = mem_bits(ty)?;
            format!("_load({addr}, {bits})")
        }
    };
    let _ = writeln!(out, "{indent}{dest} = {expr}");
    Ok(())
}

pub(super) fn emit_store_at(
    out: &mut String,
    indent: &str,
    addr: &str,
    val: &str,
    ty: &TypeRef,
) -> Result<()> {
    match ty.as_ref() {
        Type::FPType(FPType::Single) => {
            let _ = writeln!(out, "{indent}_store_f32({addr}, {val})");
        }
        Type::FPType(FPType::Double) => {
            let _ = writeln!(out, "{indent}_store_f64({addr}, {val})");
        }
        _ => {
            let bits = mem_bits(ty)?;
            let _ = writeln!(out, "{indent}_store({addr}, {val}, {bits})");
        }
    }
    Ok(())
}

pub(super) fn emit_extractvalue(
    out: &mut String,
    ev: &llvm_ir::instruction::ExtractValue,
    indent: &str,
    cx: &mut LayoutCx<'_>,
) -> Result<()> {
    let agg_ty = ev.aggregate.get_type(cx.types);
    let dest = name_to_var(&ev.dest);
    let src = operand_str(&ev.aggregate);
    let (offset, leaf_ty) = aggregate_walk(&agg_ty, &ev.indices, cx)?;
    let field = if offset == 0 { src } else { format!("{src} + {offset}") };
    if is_aggregate(&leaf_ty) {
        let leaf_size = cx.size(&leaf_ty)?;
        let _ = writeln!(out, "{indent}{dest} = _alloc({leaf_size})");
        let _ = writeln!(out, "{indent}_memcpy({dest}, {field}, {leaf_size})");
    } else {
        emit_load_at(out, indent, &dest, &field, &leaf_ty)?;
    }
    Ok(())
}

// awk is single-threaded, so atomicrmw collapses to load + op + store with
// the old value returned. We support the integer ops that show up in
// shared_ptr / atomic<int> ref-count code; floats / float-{Max,Min} bail.
pub(super) fn emit_atomicrmw(
    out: &mut String,
    a: &llvm_ir::instruction::AtomicRMW,
    indent: &str,
    types: &llvm_ir::types::Types,
) -> Result<()> {
    let addr = operand_str(&a.address);
    let val = operand_str(&a.value);
    let val_ty = a.value.get_type(types);
    let dest = name_to_var(&a.dest);
    let load_expr = match val_ty.as_ref() {
        Type::IntegerType { bits } => format!("_load({addr}, {bits})"),
        Type::PointerType { .. } => format!("_load({addr}, 64)"),
        other => bail!("atomicrmw on type {other} not supported"),
    };
    let bits = match val_ty.as_ref() {
        Type::IntegerType { bits } => *bits,
        Type::PointerType { .. } => 64,
        _ => unreachable!(),
    };
    let new_expr = match a.operation {
        RMWBinOp::Xchg => val.clone(),
        RMWBinOp::Add => format!("{dest} + {val}"),
        RMWBinOp::Sub => format!("{dest} - {val}"),
        RMWBinOp::And => format!("and({dest}, {val})"),
        RMWBinOp::Or => format!("or({dest}, {val})"),
        RMWBinOp::Xor => format!("xor({dest}, {val})"),
        RMWBinOp::Nand => format!("xor(and({dest}, {val}), -1)"),
        RMWBinOp::Max | RMWBinOp::UMax => {
            format!("({dest} > {val}) ? {dest} : {val}")
        }
        RMWBinOp::Min | RMWBinOp::UMin => {
            format!("({dest} < {val}) ? {dest} : {val}")
        }
        other => bail!("atomicrmw operation {other:?} not supported"),
    };
    let _ = writeln!(out, "{indent}{dest} = {load_expr}");
    let _ = writeln!(out, "{indent}_store({addr}, {new_expr}, {bits})");
    Ok(())
}

pub(super) fn emit_insertvalue(
    out: &mut String,
    iv: &llvm_ir::instruction::InsertValue,
    indent: &str,
    cx: &mut LayoutCx<'_>,
) -> Result<()> {
    let agg_ty = iv.aggregate.get_type(cx.types);
    let size = cx.size(&agg_ty)?;
    let dest = name_to_var(&iv.dest);
    let _ = writeln!(out, "{indent}{dest} = _alloc({size})");
    // Skip the memcpy when the source is undef/poison/zero — MEM auto-zeros
    // and the insert below will overwrite the relevant field anyway.
    if !is_undef_or_zero(&iv.aggregate) {
        let src = operand_str(&iv.aggregate);
        let _ = writeln!(out, "{indent}_memcpy({dest}, {src}, {size})");
    }
    let (offset, leaf_ty) = aggregate_walk(&agg_ty, &iv.indices, cx)?;
    let field = if offset == 0 { dest.clone() } else { format!("{dest} + {offset}") };
    let val = operand_str(&iv.element);
    let elem_ty = iv.element.get_type(cx.types);
    if is_aggregate(&elem_ty) {
        if !is_undef_or_zero(&iv.element) {
            let elem_size = cx.size(&elem_ty)?;
            let _ = writeln!(out, "{indent}_memcpy({field}, {val}, {elem_size})");
        }
    } else {
        // The IR-declared leaf type is what the slot is sized for; use it
        // rather than the element's own type in case of a width mismatch.
        let _ = leaf_ty;
        emit_store_at(out, indent, &field, &val, &elem_ty)?;
    }
    Ok(())
}

pub(super) fn emit_gep(
    out: &mut String,
    gep: &llvm_ir::instruction::GetElementPtr,
    indent: &str,
    cx: &mut LayoutCx<'_>,
) -> Result<()> {
    let dest = name_to_var(&gep.dest);
    let mut const_off: i64 = 0;
    let mut runtime: Vec<(u64, String)> = Vec::new();

    let mut current = resolve_named(gep.source_element_type.clone(), cx.types)?;
    let mut idxs = gep.indices.iter();

    // First index strides over copies of source_element_type (treats base as
    // a 1-element array of that type), but does not descend.
    let first = idxs
        .next()
        .ok_or_else(|| anyhow!("GEP must have at least one index"))?;
    let stride = cx.size(&current)?;
    accumulate_index(first, stride, &mut const_off, &mut runtime)?;

    // Remaining indices descend into aggregates.
    for idx in idxs {
        current = resolve_named(current, cx.types)?;
        match current.as_ref() {
            Type::ArrayType { element_type, .. } => {
                let element_type = element_type.clone();
                let stride = cx.size(&element_type)?;
                accumulate_index(idx, stride, &mut const_off, &mut runtime)?;
                current = element_type;
            }
            Type::StructType { element_types, is_packed } => {
                let element_types = element_types.clone();
                let is_packed = *is_packed;
                let layout = cx.struct_layout(&element_types, is_packed)?;
                let i = const_index(idx)? as usize;
                let off = *layout
                    .offsets
                    .get(i)
                    .ok_or_else(|| anyhow!("struct index out of range"))?;
                const_off += off as i64;
                current = element_types[i].clone();
            }
            other => bail!("GEP descent into {other} not supported"),
        }
    }

    let mut expr = operand_str(&gep.address);
    if const_off > 0 {
        expr = format!("{expr} + {const_off}");
    } else if const_off < 0 {
        expr = format!("{expr} - {}", -const_off);
    }
    for (stride, var) in runtime {
        if stride == 1 {
            expr = format!("{expr} + {var}");
        } else {
            expr = format!("{expr} + {stride} * {var}");
        }
    }
    let _ = writeln!(out, "{indent}{dest} = {expr}");
    Ok(())
}

// Walk an aggregate type with constant indices, returning (byte_offset,
// leaf_type). Used by extractvalue / insertvalue.
fn aggregate_walk(
    base_ty: &TypeRef,
    indices: &[u32],
    cx: &mut LayoutCx<'_>,
) -> Result<(u64, TypeRef)> {
    let mut current = resolve_named(base_ty.clone(), cx.types)?;
    let mut offset = 0u64;
    for &idx in indices {
        current = resolve_named(current, cx.types)?;
        match current.as_ref() {
            Type::ArrayType { element_type, .. } => {
                let stride = align_up(cx.size(element_type)?, cx.align(element_type)?);
                offset += stride * idx as u64;
                current = element_type.clone();
            }
            Type::StructType { element_types, is_packed } => {
                let element_types = element_types.clone();
                let is_packed = *is_packed;
                let layout = cx.struct_layout(&element_types, is_packed)?;
                let off = *layout
                    .offsets
                    .get(idx as usize)
                    .ok_or_else(|| anyhow!("aggregate index out of range"))?;
                offset += off;
                current = element_types[idx as usize].clone();
            }
            other => bail!("aggregate index into non-aggregate type {other}"),
        }
    }
    Ok((offset, current))
}

fn accumulate_index(
    op: &Operand,
    stride: u64,
    const_off: &mut i64,
    runtime: &mut Vec<(u64, String)>,
) -> Result<()> {
    match op {
        Operand::ConstantOperand(c) => match c.as_ref() {
            Constant::Int { value, bits } => {
                let v = sign_extend(*value, *bits);
                *const_off += (stride as i64) * v;
                Ok(())
            }
            other => bail!("non-integer constant GEP index: {other}"),
        },
        Operand::LocalOperand { name, .. } => {
            runtime.push((stride, name_to_var(name)));
            Ok(())
        }
        Operand::MetadataOperand => bail!("metadata operand as GEP index"),
    }
}

fn const_index(op: &Operand) -> Result<i64> {
    match op {
        Operand::ConstantOperand(c) => match c.as_ref() {
            Constant::Int { value, bits } => Ok(sign_extend(*value, *bits)),
            other => bail!("struct GEP index must be a constant int, got {other}"),
        },
        _ => bail!("struct GEP index must be a constant int"),
    }
}
