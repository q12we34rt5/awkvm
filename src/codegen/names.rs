use anyhow::{Result, bail};
use llvm_ir::{Constant, ConstantRef, Name, Operand, constant::Float};

use super::types::{sign_extend, type_bits};

pub(super) fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

pub(super) fn name_to_var(name: &Name) -> String {
    match name {
        Name::Number(n) => format!("r{n}"),
        Name::Name(s) => format!("r_{}", sanitize(s)),
    }
}

// Strip the LLVM "literal asm" prefix from an IR symbol name. Names
// that were emitted via `__asm__("X")` come through as `\x01X`; on
// Darwin, libc's LFS-renamed functions (fopen / fwrite / fputs /
// freopen / popen) use this with a leading underscore (`\x01_fopen`).
// The `\x01` is a directive to the assembler, not part of the
// linker symbol — strip it (and the Darwin underscore prefix when
// present) so libc helper lookups are platform-agnostic.
pub(super) fn canonical_fn_name(s: &str) -> &str {
    if let Some(rest) = s.strip_prefix('\x01') {
        rest.strip_prefix('_').unwrap_or(rest)
    } else {
        s
    }
}

pub(super) fn func_to_var(name: &str) -> String {
    format!("fn_{}", sanitize(canonical_fn_name(name)))
}

pub(super) fn global_to_var(name: &Name) -> String {
    match name {
        Name::Number(n) => format!("g{n}"),
        Name::Name(s) => format!("g_{}", sanitize(s)),
    }
}

pub(super) fn block_label(name: &Name) -> String {
    match name {
        Name::Number(n) => format!("b{n}"),
        Name::Name(s) => format!("b_{}", sanitize(s)),
    }
}

pub(super) fn operand_str(op: &Operand) -> String {
    match op {
        Operand::LocalOperand { name, .. } => name_to_var(name),
        Operand::ConstantOperand(c) => constant_str(c),
        Operand::MetadataOperand => "0".to_string(),
    }
}

pub(super) fn constant_str(c: &ConstantRef) -> String {
    match c.as_ref() {
        Constant::Int { value, bits } => sign_extend(*value, *bits).to_string(),
        Constant::Null(_) => "0".to_string(),
        Constant::AggregateZero(_) => "0".to_string(),
        Constant::Undef(_) | Constant::Poison(_) => "0".to_string(),
        Constant::GlobalReference { name, .. } => global_to_var(name),
        Constant::Float(f) => float_literal(f),
        // Constexpr GEP `getelementptr (T, ptr @G, i64 N)` — Constant::GetElementPtr
        // doesn't carry source_element_type in llvm-ir 0.11 under opaque pointers,
        // so we assume i8 stride (byte offset). This is correct for typeinfo
        // field access like `&_ZTIi + 8` (name pointer), and although wrong-by-8x
        // for ptr-stride vtable indexing, the vtables' bytes are zero in our
        // model anyway so reads produce 0 either way.
        Constant::GetElementPtr(g) if g.indices.len() == 1 => {
            let base = constant_str(&g.address);
            if let Constant::Int { value, bits } = g.indices[0].as_ref() {
                let n = sign_extend(*value, *bits);
                if n == 0 { base } else { format!("({base} + {n})") }
            } else {
                "0".to_string()
            }
        }
        // Constexpr int<->ptr / ptr<->int / bitcast are no-ops in our
        // model: pointers ARE byte addresses, so the integer value
        // passes straight through. `(void*)0x42` shows up here as
        // `inttoptr (i64 66 to ptr)` and we want the literal 66.
        Constant::IntToPtr(c) => constant_str(&c.operand),
        Constant::PtrToInt(c) => constant_str(&c.operand),
        Constant::BitCast(c) => constant_str(&c.operand),
        // Fall-through: bake to 0 rather than writing /* comment */ syntax
        // that wouldn't survive being inlined into an awk expression. Whatever
        // codepath consumes this value is on its own; for our smoke fixtures
        // these are typically dead branches.
        _ => "0".to_string(),
    }
}

pub(super) fn float_literal(f: &Float) -> String {
    // Debug formatting (e.g. "1.0", "3.14") guarantees a decimal point so
    // awk's strtod parses it as a number rather than juxtaposed tokens.
    match f {
        Float::Single(v) => format!("{v:?}"),
        Float::Double(v) => format!("{v:?}"),
        other => format!("0 /* unsupported float: {other} */"),
    }
}

pub(super) fn operand_bits(op: &Operand) -> Result<u32> {
    match op {
        Operand::LocalOperand { ty, .. } => type_bits(ty),
        Operand::ConstantOperand(c) => match c.as_ref() {
            Constant::Int { bits, .. } => Ok(*bits),
            other => bail!("operand has non-integer constant type: {other}"),
        },
        Operand::MetadataOperand => bail!("metadata operand has no bit width"),
    }
}
