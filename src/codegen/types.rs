use std::collections::HashMap;

use anyhow::{Result, bail};
use llvm_ir::{
    Constant, Operand, Type, TypeRef,
    types::{FPType, NamedStructDef, Types},
};

pub(super) struct StructLayout {
    pub size: u64,
    pub offsets: Vec<u64>,
}

// Type layout queries with memoization on named structs.
//
// Layout is purely a function of the type, so a single LayoutCx per emit()
// run is sound. The cache key is `NamedStructType.name` because that is
// what callers re-query repeatedly (once per field access into the same
// type). Non-named structs and arrays are cheap to recompute and skipped
// to keep the cache from growing without bound.
pub(super) struct LayoutCx<'a> {
    pub types: &'a Types,
    sizes: HashMap<String, u64>,
    aligns: HashMap<String, u64>,
}

impl<'a> LayoutCx<'a> {
    pub(super) fn new(types: &'a Types) -> Self {
        Self {
            types,
            sizes: HashMap::new(),
            aligns: HashMap::new(),
        }
    }

    pub(super) fn size(&mut self, ty: &TypeRef) -> Result<u64> {
        match ty.as_ref() {
            Type::IntegerType { bits } => Ok(((*bits as u64) + 7) / 8),
            Type::PointerType { .. } => Ok(8),
            Type::FPType(FPType::Single) => Ok(4),
            Type::FPType(FPType::Double) => Ok(8),
            Type::ArrayType { element_type, num_elements } => {
                let elem = self.size(element_type)?;
                let align = self.align(element_type)?;
                let stride = align_up(elem, align);
                Ok(stride * (*num_elements as u64))
            }
            Type::StructType { element_types, is_packed } => {
                Ok(self.struct_layout(element_types, *is_packed)?.size)
            }
            Type::NamedStructType { name } => {
                if let Some(&s) = self.sizes.get(name) {
                    return Ok(s);
                }
                let def = match self.types.named_struct_def(name) {
                    Some(NamedStructDef::Defined(d)) => d.clone(),
                    _ => bail!("opaque or undefined struct: {name}"),
                };
                let s = self.size(&def)?;
                self.sizes.insert(name.clone(), s);
                Ok(s)
            }
            other => bail!("size of type {other} not supported"),
        }
    }

    pub(super) fn align(&mut self, ty: &TypeRef) -> Result<u64> {
        match ty.as_ref() {
            Type::IntegerType { bits } => Ok((((*bits as u64) + 7) / 8).max(1)),
            Type::PointerType { .. } => Ok(8),
            Type::FPType(FPType::Single) => Ok(4),
            Type::FPType(FPType::Double) => Ok(8),
            Type::ArrayType { element_type, .. } => self.align(element_type),
            Type::StructType { element_types, is_packed } => {
                if *is_packed {
                    Ok(1)
                } else {
                    let mut max = 1u64;
                    for el in element_types {
                        let a = self.align(el)?;
                        if a > max {
                            max = a;
                        }
                    }
                    Ok(max)
                }
            }
            Type::NamedStructType { name } => {
                if let Some(&a) = self.aligns.get(name) {
                    return Ok(a);
                }
                let def = match self.types.named_struct_def(name) {
                    Some(NamedStructDef::Defined(d)) => d.clone(),
                    _ => bail!("opaque or undefined struct: {name}"),
                };
                let a = self.align(&def)?;
                self.aligns.insert(name.clone(), a);
                Ok(a)
            }
            other => bail!("alignment of type {other} not supported"),
        }
    }

    pub(super) fn struct_layout(
        &mut self,
        elements: &[TypeRef],
        packed: bool,
    ) -> Result<StructLayout> {
        let mut offsets = Vec::with_capacity(elements.len());
        let mut offset = 0u64;
        let mut max_align = 1u64;
        for el in elements {
            let sz = self.size(el)?;
            let al = if packed { 1 } else { self.align(el)? };
            offset = align_up(offset, al);
            offsets.push(offset);
            offset += sz;
            if al > max_align {
                max_align = al;
            }
        }
        let final_align = if packed { 1 } else { max_align };
        Ok(StructLayout {
            size: align_up(offset, final_align),
            offsets,
        })
    }
}

pub(super) fn sign_extend(value: u64, bits: u32) -> i64 {
    if bits >= 64 {
        return value as i64;
    }
    let shift = 64 - bits;
    ((value as i64) << shift) >> shift
}

pub(super) fn type_bits(ty: &TypeRef) -> Result<u32> {
    match ty.as_ref() {
        Type::IntegerType { bits } => Ok(*bits),
        other => bail!("expected integer type, got {other}"),
    }
}

// Bit width to use when loading/storing a value of this type. Pointers are
// fixed at 64 bits (x86_64 model).
pub(super) fn mem_bits(ty: &TypeRef) -> Result<u32> {
    match ty.as_ref() {
        Type::IntegerType { bits } => Ok(*bits),
        Type::PointerType { .. } => Ok(64),
        Type::FPType(FPType::Single) => Ok(32),
        Type::FPType(FPType::Double) => Ok(64),
        other => bail!("load/store of type {other} not supported"),
    }
}

pub(super) fn align_up(x: u64, a: u64) -> u64 {
    if a <= 1 { x } else { (x + a - 1) / a * a }
}

pub(super) fn is_aggregate(ty: &TypeRef) -> bool {
    matches!(
        ty.as_ref(),
        Type::StructType { .. } | Type::ArrayType { .. } | Type::NamedStructType { .. }
    )
}

pub(super) fn is_undef_or_zero(op: &Operand) -> bool {
    if let Operand::ConstantOperand(c) = op {
        matches!(
            c.as_ref(),
            Constant::Undef(_) | Constant::Poison(_) | Constant::AggregateZero(_)
        )
    } else {
        false
    }
}

pub(super) fn resolve_named(ty: TypeRef, types: &Types) -> Result<TypeRef> {
    let mut cur = ty;
    while let Type::NamedStructType { name } = cur.as_ref() {
        match types.named_struct_def(name) {
            Some(NamedStructDef::Defined(def)) => cur = def.clone(),
            _ => bail!("opaque or undefined struct: {name}"),
        }
    }
    Ok(cur)
}
