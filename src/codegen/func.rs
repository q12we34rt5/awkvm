use std::collections::BTreeSet;
use std::fmt::Write as _;

use anyhow::{Result, bail};
use llvm_ir::{
    Constant, Function, Instruction, IntPredicate, Name, Operand, Terminator, Type,
    predicates::FPPredicate, types::{FPType, Typed},
};

use super::call::{emit_call, emit_terminator};
use super::mem::{
    emit_atomicrmw, emit_extractvalue, emit_gep, emit_insertvalue, emit_load_at, emit_store_at,
};
use super::names::{block_label, func_to_var, name_to_var, operand_bits, operand_str};
use super::types::{LayoutCx, mem_bits, sign_extend, type_bits};

pub(super) fn emit_function(
    out: &mut String,
    func: &Function,
    cx: &mut LayoutCx<'_>,
) -> Result<()> {
    // External declarations (printf, malloc, libc++ internals, ...) have no
    // body. Most are intercepted at the call site by emit_libc. For the rest,
    // emit a no-op stub so a stray direct call doesn't crash gawk at runtime
    // — matches our "leak / tolerate" stance toward the C++ standard library.
    if func.basic_blocks.is_empty() {
        let params: Vec<String> = func
            .parameters
            .iter()
            .map(|p| name_to_var(&p.name))
            .collect();
        let _ = writeln!(
            out,
            "function {}({}) {{}}",
            func_to_var(&func.name),
            params.join(", ")
        );
        let _ = writeln!(out);
        return Ok(());
    }
    let params: Vec<String> = func.parameters.iter().map(|p| name_to_var(&p.name)).collect();
    let multi_block = func.basic_blocks.len() > 1;

    let mut locals: BTreeSet<String> = BTreeSet::new();
    let add = |name: &Name, locals: &mut BTreeSet<String>| {
        let var = name_to_var(name);
        if !params.contains(&var) {
            locals.insert(var);
        }
    };
    for bb in &func.basic_blocks {
        for instr in &bb.instrs {
            if let Some(name) = instruction_dest(instr) {
                add(name, &mut locals);
            }
        }
        // Invoke is a terminator that produces an SSA value.
        if let Terminator::Invoke(inv) = &bb.term {
            add(&inv.result, &mut locals);
        }
    }
    if multi_block {
        locals.insert("block".to_string());
    }

    let _ = write!(out, "function {}(", func_to_var(&func.name));
    let _ = write!(out, "{}", params.join(", "));
    if !locals.is_empty() {
        let sep = if params.is_empty() { "    " } else { ",    " };
        let locals_csv = locals.iter().cloned().collect::<Vec<_>>().join(", ");
        let _ = write!(out, "{sep}{locals_csv}");
    }
    let _ = writeln!(out, ") {{");

    if multi_block {
        emit_multi_block(out, func, cx)?;
    } else {
        let bb = &func.basic_blocks[0];
        for instr in &bb.instrs {
            emit_instruction(out, instr, "    ", cx)?;
        }
        emit_terminator(out, &bb.term, &bb.name, func, "    ", cx)?;
    }

    let _ = writeln!(out, "}}");
    let _ = writeln!(out);
    Ok(())
}

fn emit_multi_block(out: &mut String, func: &Function, cx: &mut LayoutCx<'_>) -> Result<()> {
    let entry = &func.basic_blocks[0];
    let _ = writeln!(out, "    block = \"{}\"", block_label(&entry.name));
    let _ = writeln!(out, "    while (1) {{");
    for (i, bb) in func.basic_blocks.iter().enumerate() {
        let kw = if i == 0 { "if" } else { "else if" };
        let _ = writeln!(out, "        {kw} (block == \"{}\") {{", block_label(&bb.name));
        for instr in &bb.instrs {
            emit_instruction(out, instr, "            ", cx)?;
        }
        emit_terminator(out, &bb.term, &bb.name, func, "            ", cx)?;
        let _ = writeln!(out, "        }}");
    }
    let _ = writeln!(out, "    }}");
    Ok(())
}

fn instruction_dest(instr: &Instruction) -> Option<&Name> {
    use Instruction::*;
    match instr {
        Add(i) => Some(&i.dest),
        Sub(i) => Some(&i.dest),
        Mul(i) => Some(&i.dest),
        SDiv(i) => Some(&i.dest),
        SRem(i) => Some(&i.dest),
        UDiv(i) => Some(&i.dest),
        URem(i) => Some(&i.dest),
        Shl(i) => Some(&i.dest),
        LShr(i) => Some(&i.dest),
        AShr(i) => Some(&i.dest),
        And(i) => Some(&i.dest),
        Or(i) => Some(&i.dest),
        Xor(i) => Some(&i.dest),
        ZExt(i) => Some(&i.dest),
        SExt(i) => Some(&i.dest),
        Trunc(i) => Some(&i.dest),
        Select(i) => Some(&i.dest),
        ICmp(i) => Some(&i.dest),
        Phi(i) => Some(&i.dest),
        Call(i) => i.dest.as_ref(),
        Alloca(i) => Some(&i.dest),
        Load(i) => Some(&i.dest),
        GetElementPtr(i) => Some(&i.dest),
        BitCast(i) => Some(&i.dest),
        IntToPtr(i) => Some(&i.dest),
        PtrToInt(i) => Some(&i.dest),
        FAdd(i) => Some(&i.dest),
        FSub(i) => Some(&i.dest),
        FMul(i) => Some(&i.dest),
        FDiv(i) => Some(&i.dest),
        FRem(i) => Some(&i.dest),
        FNeg(i) => Some(&i.dest),
        FCmp(i) => Some(&i.dest),
        SIToFP(i) => Some(&i.dest),
        UIToFP(i) => Some(&i.dest),
        FPToSI(i) => Some(&i.dest),
        FPToUI(i) => Some(&i.dest),
        FPExt(i) => Some(&i.dest),
        FPTrunc(i) => Some(&i.dest),
        ExtractValue(i) => Some(&i.dest),
        InsertValue(i) => Some(&i.dest),
        LandingPad(i) => Some(&i.dest),
        AtomicRMW(i) => Some(&i.dest),
        Freeze(i) => Some(&i.dest),
        _ => None,
    }
}

fn emit_instruction(
    out: &mut String,
    instr: &Instruction,
    indent: &str,
    cx: &mut LayoutCx<'_>,
) -> Result<()> {
    use Instruction::*;
    match instr {
        Add(i) => binop(out, indent, &i.dest, &i.operand0, "+", &i.operand1),
        Sub(i) => binop(out, indent, &i.dest, &i.operand0, "-", &i.operand1),
        // Mul can overflow the IR-typed width by far more than one bit
        // (e.g. i32 * i32 → up to ±2^62), and downstream bitwise helpers
        // (`_xor`, `_lshr` …) only normalize values within ±2^w of zero.
        // mt19937's seed mixer (`* 0x9e3779b9` and `* 1812433253`) walks
        // straight off the cliff without this. Add/Sub stay close enough
        // to width that the existing bitwise normalizers absorb them.
        Mul(i) => {
            let bits = operand_bits(&i.operand0)?;
            let _ = writeln!(
                out,
                "{indent}{} = _trunc({} * {}, {bits})",
                name_to_var(&i.dest),
                operand_str(&i.operand0),
                operand_str(&i.operand1),
            );
            Ok(())
        }
        SDiv(i) => {
            let _ = writeln!(
                out,
                "{indent}{} = int({} / {})",
                name_to_var(&i.dest),
                operand_str(&i.operand0),
                operand_str(&i.operand1)
            );
            Ok(())
        }
        SRem(i) => {
            let dest = name_to_var(&i.dest);
            let a = operand_str(&i.operand0);
            let b = operand_str(&i.operand1);
            let _ = writeln!(out, "{indent}{dest} = {a} - int({a} / {b}) * {b}");
            Ok(())
        }
        UDiv(i) => emit_bitwise(out, indent, &i.dest, "_udiv", &i.operand0, &i.operand1),
        URem(i) => emit_bitwise(out, indent, &i.dest, "_urem", &i.operand0, &i.operand1),
        Shl(i) => emit_bitwise(out, indent, &i.dest, "_shl", &i.operand0, &i.operand1),
        LShr(i) => emit_bitwise(out, indent, &i.dest, "_lshr", &i.operand0, &i.operand1),
        AShr(i) => fn_call(out, indent, &i.dest, "_ashr", &[&i.operand0, &i.operand1]),
        And(i) => emit_bitwise(out, indent, &i.dest, "_and", &i.operand0, &i.operand1),
        Or(i) => emit_bitwise(out, indent, &i.dest, "_or", &i.operand0, &i.operand1),
        Xor(i) => emit_bitwise(out, indent, &i.dest, "_xor", &i.operand0, &i.operand1),
        ZExt(i) => {
            let from_bits = operand_bits(&i.operand)?;
            let _ = writeln!(
                out,
                "{indent}{} = _zext({}, {from_bits})",
                name_to_var(&i.dest),
                operand_str(&i.operand)
            );
            Ok(())
        }
        SExt(i) => {
            // Our integer encoding is already sign-extended at all widths,
            // so widening is a no-op.
            let _ = writeln!(
                out,
                "{indent}{} = {}",
                name_to_var(&i.dest),
                operand_str(&i.operand)
            );
            Ok(())
        }
        Trunc(i) => {
            let to_bits = type_bits(&i.to_type)?;
            let _ = writeln!(
                out,
                "{indent}{} = _trunc({}, {to_bits})",
                name_to_var(&i.dest),
                operand_str(&i.operand)
            );
            Ok(())
        }
        Select(i) => {
            let _ = writeln!(
                out,
                "{indent}{} = {} ? {} : {}",
                name_to_var(&i.dest),
                operand_str(&i.condition),
                operand_str(&i.true_value),
                operand_str(&i.false_value),
            );
            Ok(())
        }
        ICmp(i) => emit_icmp(out, indent, &i.dest, i.predicate, &i.operand0, &i.operand1),
        Phi(_) => Ok(()),
        Call(call) => emit_call(out, call, indent, cx),
        Alloca(a) => {
            let elem_size = cx.size(&a.allocated_type)?;
            let dest = name_to_var(&a.dest);
            let size_expr = match &a.num_elements {
                Operand::ConstantOperand(c) => match c.as_ref() {
                    Constant::Int { value, bits } => {
                        let n = sign_extend(*value, *bits);
                        format!("{}", elem_size as i64 * n)
                    }
                    _ => format!("{elem_size} * {}", operand_str(&a.num_elements)),
                },
                _ => format!("{elem_size} * {}", operand_str(&a.num_elements)),
            };
            let _ = writeln!(out, "{indent}{dest} = _alloc({size_expr})");
            Ok(())
        }
        Load(l) => {
            let dest = name_to_var(&l.dest);
            let addr = operand_str(&l.address);
            emit_load_at(out, indent, &dest, &addr, &l.loaded_ty)
        }
        Store(s) => {
            let addr = operand_str(&s.address);
            let val = operand_str(&s.value);
            let val_ty = s.value.get_type(cx.types);
            emit_store_at(out, indent, &addr, &val, &val_ty)
        }
        ExtractValue(ev) => emit_extractvalue(out, ev, indent, cx),
        InsertValue(iv) => emit_insertvalue(out, iv, indent, cx),
        AtomicRMW(a) => emit_atomicrmw(out, a, indent, cx.types),
        Freeze(f) => {
            // Identity for non-poison/undef inputs (the only case we model).
            let _ = writeln!(
                out,
                "{indent}{} = {}",
                name_to_var(&f.dest),
                operand_str(&f.operand)
            );
            Ok(())
        }
        LandingPad(lp) => {
            // Materialize the {ptr, i32} aggregate from the global EXC_*
            // state set by __cxa_throw / Resume. Reaching landingpad means
            // we're handling, so clear UNWINDING.
            let dest = name_to_var(&lp.dest);
            let size = cx.size(&lp.result_type)?;
            let _ = writeln!(out, "{indent}{dest} = _alloc({size})");
            let _ = writeln!(out, "{indent}_store({dest}, EXC_OBJ, 64)");
            let _ = writeln!(out, "{indent}_store({dest} + 8, EXC_TYPE_ID, 32)");
            let _ = writeln!(out, "{indent}UNWINDING = 0");
            Ok(())
        }
        GetElementPtr(g) => emit_gep(out, g, indent, cx),
        BitCast(c) => {
            let dest = name_to_var(&c.dest);
            let val = operand_str(&c.operand);
            let src_ty = c.operand.get_type(cx.types);
            let src_is_fp = matches!(src_ty.as_ref(), Type::FPType(_));
            let dst_is_fp = matches!(c.to_type.as_ref(), Type::FPType(_));
            // Same kind on both sides: pure no-op (awk has one number type;
            // pointers are already byte addresses).
            let expr = match (src_is_fp, dst_is_fp) {
                (false, false) | (true, true) => val,
                (false, true) => match c.to_type.as_ref() {
                    Type::FPType(FPType::Single) => format!("_f32_from_bits({val})"),
                    Type::FPType(FPType::Double) => {
                        bail!("bitcast i64<->double not supported (would exceed awk's 53-bit safe int range)")
                    }
                    _ => unreachable!(),
                },
                (true, false) => match src_ty.as_ref() {
                    Type::FPType(FPType::Single) => format!("_f32_to_bits({val})"),
                    Type::FPType(FPType::Double) => {
                        bail!("bitcast double<->i64 not supported (would exceed awk's 53-bit safe int range)")
                    }
                    _ => unreachable!(),
                },
            };
            let _ = writeln!(out, "{indent}{dest} = {expr}");
            Ok(())
        }
        IntToPtr(c) => {
            let _ = writeln!(
                out,
                "{indent}{} = {}",
                name_to_var(&c.dest),
                operand_str(&c.operand)
            );
            Ok(())
        }
        PtrToInt(c) => {
            let _ = writeln!(
                out,
                "{indent}{} = {}",
                name_to_var(&c.dest),
                operand_str(&c.operand)
            );
            Ok(())
        }
        FAdd(i) => binop(out, indent, &i.dest, &i.operand0, "+", &i.operand1),
        FSub(i) => binop(out, indent, &i.dest, &i.operand0, "-", &i.operand1),
        FMul(i) => binop(out, indent, &i.dest, &i.operand0, "*", &i.operand1),
        FDiv(i) => binop(out, indent, &i.dest, &i.operand0, "/", &i.operand1),
        // Float remainder: x - trunc(x / y) * y. Sign follows the
        // dividend (matches C99 `fmod`, which is what `std::fmod`
        // lowers to via `frem`). gawk's `int()` truncates toward zero,
        // so `int(x / y)` is exactly the trunc we want.
        FRem(i) => {
            let a = operand_str(&i.operand0);
            let b = operand_str(&i.operand1);
            let _ = writeln!(
                out,
                "{indent}{} = {a} - int({a} / {b}) * {b}",
                name_to_var(&i.dest),
            );
            Ok(())
        }
        FNeg(i) => {
            let _ = writeln!(
                out,
                "{indent}{} = -{}",
                name_to_var(&i.dest),
                operand_str(&i.operand)
            );
            Ok(())
        }
        FCmp(i) => emit_fcmp(out, indent, &i.dest, &i.operand0, i.predicate, &i.operand1),
        // awk has a single numeric type, so int<->float conversions outside
        // memory are no-ops apart from truncation toward zero for f->i.
        SIToFP(i) => fp_noop(out, indent, &i.dest, &i.operand),
        UIToFP(i) => {
            // Operand is signed in our model (sign-extended); reinterpret as
            // unsigned before promoting to float, otherwise an i8 byte 0xFF
            // (= -1 in our world) becomes -1.0 instead of 255.0.
            let bits = operand_bits(&i.operand)?;
            let _ = writeln!(
                out,
                "{indent}{} = _zext({}, {bits})",
                name_to_var(&i.dest),
                operand_str(&i.operand)
            );
            Ok(())
        }
        FPToSI(i) => fp_to_int(out, indent, &i.dest, &i.operand),
        FPToUI(i) => fp_to_int(out, indent, &i.dest, &i.operand),
        FPExt(i) => fp_noop(out, indent, &i.dest, &i.operand),
        FPTrunc(i) => fp_noop(out, indent, &i.dest, &i.operand),
        other => bail!("instruction not implemented: {other}"),
    }
}

fn icmp_op(p: IntPredicate) -> &'static str {
    match p {
        IntPredicate::EQ => "==",
        IntPredicate::NE => "!=",
        IntPredicate::SGT | IntPredicate::UGT => ">",
        IntPredicate::SGE | IntPredicate::UGE => ">=",
        IntPredicate::SLT | IntPredicate::ULT => "<",
        IntPredicate::SLE | IntPredicate::ULE => "<=",
    }
}

fn icmp_is_unsigned(p: IntPredicate) -> bool {
    matches!(
        p,
        IntPredicate::UGT | IntPredicate::UGE | IntPredicate::ULT | IntPredicate::ULE
    )
}

fn emit_icmp(
    out: &mut String,
    indent: &str,
    dest: &Name,
    p: IntPredicate,
    lhs: &Operand,
    rhs: &Operand,
) -> Result<()> {
    let op = icmp_op(p);
    let dest = name_to_var(dest);
    let lhs_s = operand_str(lhs);
    let rhs_s = operand_str(rhs);
    if icmp_is_unsigned(p) {
        // Reinterpret both sides as unsigned at the operand's bit width so
        // negative values (which represent high-bit-set bit patterns in our
        // signed model) compare in the right order. Pointers count as 64-bit
        // for this purpose (mem_bits, not operand_bits).
        let bits = match lhs {
            Operand::LocalOperand { ty, .. } => mem_bits(ty)?,
            _ => operand_bits(lhs)?,
        };
        let _ = writeln!(
            out,
            "{indent}{dest} = _zext({lhs_s}, {bits}) {op} _zext({rhs_s}, {bits})"
        );
    } else {
        let _ = writeln!(out, "{indent}{dest} = {lhs_s} {op} {rhs_s}");
    }
    Ok(())
}

fn emit_bitwise(
    out: &mut String,
    indent: &str,
    dest: &Name,
    helper: &str,
    lhs: &Operand,
    rhs: &Operand,
) -> Result<()> {
    let bits = operand_bits(lhs)?;
    let _ = writeln!(
        out,
        "{indent}{} = {helper}({}, {}, {bits})",
        name_to_var(dest),
        operand_str(lhs),
        operand_str(rhs)
    );
    Ok(())
}

fn fp_noop(out: &mut String, indent: &str, dest: &Name, op: &Operand) -> Result<()> {
    let _ = writeln!(out, "{indent}{} = {}", name_to_var(dest), operand_str(op));
    Ok(())
}

fn fp_to_int(out: &mut String, indent: &str, dest: &Name, op: &Operand) -> Result<()> {
    let _ = writeln!(out, "{indent}{} = int({})", name_to_var(dest), operand_str(op));
    Ok(())
}

// awk has no NaN; ordered and unordered predicates collapse to the same
// awk operator. ord/uno are baked to their NaN-free answers.
fn emit_fcmp(
    out: &mut String,
    indent: &str,
    dest: &Name,
    lhs: &Operand,
    p: FPPredicate,
    rhs: &Operand,
) -> Result<()> {
    let dest = name_to_var(dest);
    let a = operand_str(lhs);
    let b = operand_str(rhs);
    let expr = match p {
        FPPredicate::False => "0".to_string(),
        FPPredicate::True | FPPredicate::ORD => "1".to_string(),
        FPPredicate::UNO => "0".to_string(),
        FPPredicate::OEQ | FPPredicate::UEQ => format!("{a} == {b}"),
        FPPredicate::ONE | FPPredicate::UNE => format!("{a} != {b}"),
        FPPredicate::OGT | FPPredicate::UGT => format!("{a} > {b}"),
        FPPredicate::OGE | FPPredicate::UGE => format!("{a} >= {b}"),
        FPPredicate::OLT | FPPredicate::ULT => format!("{a} < {b}"),
        FPPredicate::OLE | FPPredicate::ULE => format!("{a} <= {b}"),
    };
    let _ = writeln!(out, "{indent}{dest} = {expr}");
    Ok(())
}

fn binop(
    out: &mut String,
    indent: &str,
    dest: &Name,
    lhs: &Operand,
    op: &str,
    rhs: &Operand,
) -> Result<()> {
    let _ = writeln!(
        out,
        "{indent}{} = {} {op} {}",
        name_to_var(dest),
        operand_str(lhs),
        operand_str(rhs)
    );
    Ok(())
}

fn fn_call(
    out: &mut String,
    indent: &str,
    dest: &Name,
    func: &str,
    args: &[&Operand],
) -> Result<()> {
    let args_csv = args.iter().map(|a| operand_str(a)).collect::<Vec<_>>().join(", ");
    let _ = writeln!(out, "{indent}{} = {func}({args_csv})", name_to_var(dest));
    Ok(())
}
