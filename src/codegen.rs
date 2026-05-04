use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;

const MAX_ICALL_ARITY: usize = 8;

use anyhow::{Result, anyhow, bail};
use either::Either;
use llvm_ir::{
    Constant, ConstantRef, Function, Instruction, IntPredicate, Module, Name, Operand, Terminator,
    Type, TypeRef,
    constant::Float,
    instruction::RMWBinOp,
    predicates::FPPredicate,
    types::{FPType, NamedStructDef, Typed, Types},
};

const RUNTIME: &str = r#"BEGIN { NEXT_ADDR = 1 }
function _ashr(a, n) {
    return (a < 0 && a % (2 ^ n) != 0) ? int(a / (2 ^ n)) - 1 : int(a / (2 ^ n))
}
function _zext(a, w) {
    return a < 0 ? a + (2 ^ w) : a
}
function _trunc(a, w,    m, mod) {
    mod = 2 ^ w
    m = a % mod
    if (m < 0) m += mod
    return m >= mod / 2 ? m - mod : m
}
# Bitwise wrappers that accept signed operands. gawk's and/or/xor reject
# negative inputs, so zext both sides to unsigned, run the op, then
# reinterpret the result back at width w.
function _and(a, b, w,    ua, ub, r) {
    ua = a < 0 ? a + 2 ^ w : a
    ub = b < 0 ? b + 2 ^ w : b
    r = and(ua, ub)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
function _or(a, b, w,    ua, ub, r) {
    ua = a < 0 ? a + 2 ^ w : a
    ub = b < 0 ? b + 2 ^ w : b
    r = or(ua, ub)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
function _xor(a, b, w,    ua, ub, r) {
    ua = a < 0 ? a + 2 ^ w : a
    ub = b < 0 ? b + 2 ^ w : b
    r = xor(ua, ub)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
function _shl(a, n, w,    ua, r) {
    ua = a < 0 ? a + 2 ^ w : a
    r = lshift(ua, n) % (2 ^ w)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
function _lshr(a, n, w,    ua, r) {
    ua = a < 0 ? a + 2 ^ w : a
    r = rshift(ua, n)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
function _alloc(size,    a) {
    a = NEXT_ADDR
    NEXT_ADDR += size
    return a
}
# Little-endian load: read ceil(bits/8) bytes from MEM and sign-extend.
# Undefined keys read as 0, so unwritten memory acts as zero-padded.
function _load(addr, bits,    n, i, v, sign) {
    n = int((bits + 7) / 8)
    v = 0
    for (i = 0; i < n; i++) {
        v += MEM[addr + i] * (256 ^ i)
    }
    sign = 2 ^ (bits - 1)
    return v >= sign ? v - 2 * sign : v
}
function _store(addr, val, bits,    n, i, u) {
    n = int((bits + 7) / 8)
    u = val < 0 ? val + 2 ^ bits : val
    for (i = 0; i < n; i++) {
        MEM[addr + i] = u % 256
        u = int(u / 256)
    }
}
# Read a NUL-terminated byte string from MEM into an awk string.
function _cstr(addr,    s, b) {
    s = ""
    while ((b = MEM[addr]) != 0) {
        s = s sprintf("%c", b)
        addr++
    }
    return s
}
# Walks the format string, forwarding each spec to gawk's printf and pulling
# args from the global _PA array (filled by the caller before invocation).
# Supports %d %i %u %x %X %o %c %s %p and %% (no float specifiers yet).
function _printf(fmt_addr,    fmt, n, i, c, ai, j, conv, spec) {
    fmt = _cstr(fmt_addr)
    n = length(fmt)
    ai = 0
    i = 1
    while (i <= n) {
        c = substr(fmt, i, 1)
        if (c != "%") {
            printf "%s", c
            i++
            continue
        }
        j = i + 1
        while (j <= n && index("diuxXocspfFgGeE%", substr(fmt, j, 1)) == 0) {
            j++
        }
        if (j > n) {
            printf "%s", substr(fmt, i)
            break
        }
        conv = substr(fmt, j, 1)
        spec = substr(fmt, i, j - i + 1)
        if (conv == "%") {
            printf "%%"
        } else if (conv == "s") {
            printf spec, _cstr(_PA[ai]); ai++
        } else if (conv == "p") {
            printf "0x%x", _PA[ai]; ai++
        } else {
            printf spec, _PA[ai]; ai++
        }
        i = j + 1
    }
    return 0
}
function _memcpy(dst, src, n,    i) {
    for (i = 0; i < n; i++) MEM[dst + i] = MEM[src + i]
    return dst
}
function _memmove(dst, src, n,    i) {
    if (dst < src) {
        for (i = 0; i < n; i++) MEM[dst + i] = MEM[src + i]
    } else {
        for (i = n - 1; i >= 0; i--) MEM[dst + i] = MEM[src + i]
    }
    return dst
}
function _memset(p, v, n,    i, b) {
    b = v < 0 ? v + 256 : v
    b = b % 256
    for (i = 0; i < n; i++) MEM[p + i] = b
    return p
}
function _strlen(p,    n) {
    n = 0
    while (MEM[p + n] != 0) n++
    return n
}
function _strcmp(a, b,    ca, cb) {
    while (1) {
        ca = MEM[a]; cb = MEM[b]
        if (ca != cb) return ca - cb
        if (ca == 0) return 0
        a++; b++
    }
}
# Read a NUL-terminated byte string into an awk string and let awk's
# implicit string -> number conversion do the parsing. Matches C atof
# semantics for the leading numeric prefix (sign, decimals, exponent).
function _atof(addr,    s, b) {
    s = ""
    while ((b = MEM[addr]) != 0) {
        s = s sprintf("%c", b)
        addr++
    }
    return s + 0
}
function _atoi(addr) { return int(_atof(addr)) }
# Build a C-style char** argv from awk's ARGV[]. Each ARGV[i] is copied
# byte-by-byte into MEM (NUL-terminated), and pointers are packed into
# an `argc + 1` slot table (last slot is the NULL sentinel). Requires
# LC_ALL=C so substr/length operate on bytes, not multibyte characters.
function _build_argv(    i, j, s, len, ptr, table, _ord) {
    for (i = 0; i < 256; i++) _ord[sprintf("%c", i)] = i
    table = _alloc((ARGC + 1) * 8)
    for (i = 0; i < ARGC; i++) {
        s = ARGV[i]
        len = length(s)
        ptr = _alloc(len + 1)
        for (j = 0; j < len; j++) {
            MEM[ptr + j] = _ord[substr(s, j + 1, 1)]
        }
        MEM[ptr + len] = 0
        _store(table + i * 8, ptr, 64)
    }
    _store(table + ARGC * 8, 0, 64)
    return table
}
# Darwin libc: memset_pattern{4,8,16} fill `n` bytes of `dst` by repeating
# the byte pattern at `pat`. Used by clang as a memset optimization.
function _memset_pattern(dst, pat, n, plen,    i) {
    for (i = 0; i < n; i++) MEM[dst + i] = MEM[pat + (i % plen)]
}
# C++ catch-clause matching. EXC_TYPE_ID holds the thrown typeinfo's
# address; PARENT_TI[ti] gives the parent under Itanium __si_class_type_info.
# When the catch clause is compatible, return EXC_TYPE_ID so the IR's
# `icmp eq selector, eh.typeid.for(catch_ti)` matches; else -1.
function _typeid_for(catch_ti,    cur) {
    cur = EXC_TYPE_ID
    while (cur != 0) {
        if (cur == catch_ti) return EXC_TYPE_ID
        cur = (cur in PARENT_TI) ? PARENT_TI[cur] : 0
    }
    return -1
}
# IEEE 754 single-precision pack/unpack. Subnormals collapse to 0 and
# inf / NaN are not preserved (Phase 8 limitation).
function _f32_to_bits(v,    sign, av, e, m) {
    if (v == 0) return 0
    if (v < 0) { sign = 1; av = -v } else { sign = 0; av = v }
    e = 0
    while (av >= 2) { av /= 2; e++ }
    while (av < 1) { av *= 2; e-- }
    m = int((av - 1) * 8388608)
    if (e + 127 <= 0) return sign * 2147483648
    if (e + 127 >= 255) return sign * 2147483648 + 2139095040
    return sign * 2147483648 + (e + 127) * 8388608 + m
}
function _f32_from_bits(raw,    sign, bexp, mant) {
    if (raw < 0) raw += 4294967296
    sign = int(raw / 2147483648) % 2
    bexp = int(raw / 8388608) % 256
    mant = raw % 8388608
    if (bexp == 0)   return 0
    if (bexp == 255) return 0
    return (sign ? -1 : 1) * (1 + mant / 8388608) * (2 ^ (bexp - 127))
}
function _load_f32(addr) { return _f32_from_bits(_load(addr, 32)) }
function _store_f32(addr, v) { _store(addr, _f32_to_bits(v), 32) }
# IEEE 754 double-precision via two halves; the 64-bit raw pattern
# wouldn't fit in awk's 53-bit-safe integer range so we never assemble it.
function _load_f64(addr,    lo, hi, sign, bexp, mhi, m) {
    lo = _load(addr, 32);     if (lo < 0) lo += 4294967296
    hi = _load(addr + 4, 32); if (hi < 0) hi += 4294967296
    sign = int(hi / 2147483648) % 2
    bexp = int(hi / 1048576) % 2048
    mhi  = hi % 1048576
    m = mhi * 4294967296 + lo
    if (bexp == 0)    return 0
    if (bexp == 2047) return 0
    return (sign ? -1 : 1) * (1 + m / 4503599627370496) * (2 ^ (bexp - 1023))
}
function _store_f64(addr, v,    sign, av, e, m, mhi, mlo) {
    if (v == 0) { _store(addr, 0, 32); _store(addr + 4, 0, 32); return }
    if (v < 0) { sign = 1; av = -v } else { sign = 0; av = v }
    e = 0
    while (av >= 2) { av /= 2; e++ }
    while (av < 1) { av *= 2; e-- }
    m = (av - 1) * 4503599627370496
    mhi = int(m / 4294967296)
    mlo = int(m - mhi * 4294967296)
    if (e + 1023 <= 0) {
        _store(addr, 0, 32); _store(addr + 4, sign * 2147483648, 32); return
    }
    if (e + 1023 >= 2047) {
        _store(addr, 0, 32); _store(addr + 4, sign * 2147483648 + 2146435072, 32); return
    }
    _store(addr, mlo, 32)
    _store(addr + 4, sign * 2147483648 + (e + 1023) * 1048576 + mhi, 32)
}
"#;

pub fn emit(module: &Module) -> Result<String> {
    let mut out = String::new();
    let _ = writeln!(out, "# Generated by awkvm from {}", module.name);
    let _ = writeln!(out, "# Requires gawk (uses and/or/xor/lshift/rshift built-ins)");
    let _ = writeln!(out);

    let _ = writeln!(out, "{RUNTIME}");

    let fn_ids = build_fn_ids(module);

    emit_globals_init(&mut out, module, &fn_ids)?;

    if let Some(main_fn) = module.functions.iter().find(|f| f.name == "main") {
        match main_fn.parameters.len() {
            0 => {
                let _ = writeln!(out, "BEGIN {{ exit fn_main() }}");
            }
            2 => {
                // int main(int argc, char** argv)
                let _ = writeln!(out, "BEGIN {{ exit fn_main(ARGC, _build_argv()) }}");
            }
            3 => {
                // int main(int argc, char** argv, char** envp) — we don't
                // model env, so pass NULL.
                let _ = writeln!(out, "BEGIN {{ exit fn_main(ARGC, _build_argv(), 0) }}");
            }
            n => bail!("main has {n} parameters; only 0, 2, or 3 are supported"),
        }
        let _ = writeln!(out);
    }

    for func in &module.functions {
        emit_function(&mut out, func, &module.types)?;
    }

    // Stubs for `declare`-only functions (libc++ internals, etc.). Most are
    // intercepted by emit_libc at the call site, but if a stray direct call
    // slips through we want it to no-op rather than crash gawk at runtime.
    for decl in &module.func_declarations {
        let params: Vec<String> = decl
            .parameters
            .iter()
            .map(|p| name_to_var(&p.name))
            .collect();
        let _ = writeln!(
            &mut out,
            "function {}({}) {{}}",
            func_to_var(&decl.name),
            params.join(", ")
        );
    }
    let _ = writeln!(&mut out);

    emit_icall_dispatcher(&mut out, module, &fn_ids)?;
    Ok(out)
}

// Assign sequential ids (starting at 1; 0 reserved for null) to every defined
// function. Declared-only functions are skipped — they have no awk body.
fn build_fn_ids(module: &Module) -> HashMap<String, i64> {
    module
        .functions
        .iter()
        .filter(|f| !f.basic_blocks.is_empty())
        .enumerate()
        .map(|(i, f)| (f.name.clone(), (i + 1) as i64))
        .collect()
}

fn emit_icall_dispatcher(
    out: &mut String,
    module: &Module,
    fn_ids: &HashMap<String, i64>,
) -> Result<()> {
    if fn_ids.is_empty() {
        return Ok(());
    }
    // Dispatch in id order so the output is stable regardless of HashMap layout.
    let mut entries: Vec<_> = module
        .functions
        .iter()
        .filter_map(|f| fn_ids.get(&f.name).map(|id| (f, *id)))
        .collect();
    entries.sort_by_key(|(_, id)| *id);

    for (f, _) in &entries {
        if f.parameters.len() > MAX_ICALL_ARITY {
            bail!(
                "function `{}` has {} parameters; indirect dispatcher only supports up to {}",
                f.name,
                f.parameters.len(),
                MAX_ICALL_ARITY
            );
        }
    }

    let slots: Vec<String> = (0..MAX_ICALL_ARITY).map(|i| format!("a{i}")).collect();
    let _ = writeln!(out, "function _icall(fp, {}) {{", slots.join(", "));
    for (i, (f, id)) in entries.iter().enumerate() {
        let kw = if i == 0 { "if" } else { "else if" };
        let target = format!("fn_{}", sanitize(&f.name));
        let pass = slots[..f.parameters.len()].join(", ");
        let _ = writeln!(out, "    {kw} (fp == {id}) return {target}({pass})");
    }
    let _ = writeln!(out, "    else {{");
    let _ = writeln!(
        out,
        "        printf \"awkvm: indirect call to unknown function id %d\\n\", fp > \"/dev/stderr\""
    );
    let _ = writeln!(out, "        exit 1");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "}}");
    Ok(())
}

fn emit_globals_init(
    out: &mut String,
    module: &Module,
    fn_ids: &HashMap<String, i64>,
) -> Result<()> {
    // gv.ty is the pointer type (`ptr` under opaque pointers); the storage
    // type comes from the initializer.
    let with_init: Vec<_> = module
        .global_vars
        .iter()
        .filter_map(|gv| gv.initializer.as_ref().map(|i| (gv, i)))
        .collect();
    // External globals (no initializer) — typeinfo objects from libc++abi
    // mostly. They need a stable address so identity comparison works; the
    // 8-byte sentinel content is irrelevant for typeid matching.
    let externs: Vec<_> = module
        .global_vars
        .iter()
        .filter(|gv| gv.initializer.is_none())
        .collect();
    if with_init.is_empty() && externs.is_empty() && fn_ids.is_empty() {
        return Ok(());
    }
    let _ = writeln!(out, "BEGIN {{");
    // Two passes: allocate all globals first so initializers may reference
    // any global address (forward refs are common with .str pools).
    for (gv, init) in &with_init {
        let storage_ty = init.get_type(&module.types);
        let size = type_size_bytes(&storage_ty, &module.types)?;
        let _ = writeln!(out, "    {} = _alloc({size})", global_to_var(&gv.name));
    }
    for gv in &externs {
        let _ = writeln!(out, "    {} = _alloc(8)", global_to_var(&gv.name));
    }
    // Function pointer registry has to be set BEFORE emit_const_init runs:
    // vtable initializers reference functions via g_<name>, and we need that
    // awk variable to already hold the function id at the moment _store
    // captures the value. Otherwise the slot gets written as 0/empty and
    // every virtual call later dispatches to fp=0.
    let mut fns: Vec<_> = module
        .functions
        .iter()
        .filter_map(|f| fn_ids.get(&f.name).map(|id| (f.name.as_str(), *id)))
        .collect();
    fns.sort_by_key(|(_, id)| *id);
    for (name, id) in fns {
        let _ = writeln!(out, "    g_{} = {id}", sanitize(name));
    }
    for (gv, init) in &with_init {
        let storage_ty = init.get_type(&module.types);
        emit_const_init(
            out,
            &global_to_var(&gv.name),
            init,
            &storage_ty,
            &module.types,
            "    ",
        )?;
    }
    // C++ typeinfo parent registry: lets _typeid_for walk the inheritance
    // chain at catch time. We only handle Itanium __si_class_type_info
    // (single inheritance, 3-field struct: vtable, name, parent_typeinfo).
    for gv in &module.global_vars {
        let Name::Name(this_name) = &gv.name else { continue };
        if !this_name.starts_with("_ZTI") {
            continue;
        }
        let Some(init) = &gv.initializer else { continue };
        let Constant::Struct { values, .. } = init.as_ref() else { continue };
        if values.len() != 3 {
            continue; // 2-field = no parent; 4+ = vmi (multi/virtual), unsupported
        }
        let Constant::GlobalReference { name: parent_name, .. } = values[2].as_ref() else { continue };
        let Name::Name(parent_str) = parent_name else { continue };
        if !parent_str.starts_with("_ZTI") {
            continue;
        }
        let _ = writeln!(
            out,
            "    PARENT_TI[g_{}] = g_{}",
            sanitize(this_name),
            sanitize(parent_str)
        );
    }
    let _ = writeln!(out, "}}");
    let _ = writeln!(out);
    Ok(())
}

fn emit_const_init(
    out: &mut String,
    addr: &str,
    c: &ConstantRef,
    ty: &TypeRef,
    types: &Types,
    indent: &str,
) -> Result<()> {
    let sub_addr = |off: u64| -> String {
        if off == 0 {
            addr.to_string()
        } else {
            format!("{addr} + {off}")
        }
    };
    match c.as_ref() {
        Constant::Int { value, bits } => {
            let v = sign_extend(*value, *bits);
            let _ = writeln!(out, "{indent}_store({addr}, {v}, {bits})");
        }
        Constant::Null(_) => {
            let _ = writeln!(out, "{indent}_store({addr}, 0, 64)");
        }
        // MEM defaults to 0, so zero-init is a no-op.
        Constant::AggregateZero(_) | Constant::Undef(_) | Constant::Poison(_) => {}
        Constant::Array { element_type, elements } => {
            let elem_size = type_size_bytes(element_type, types)?;
            let elem_align = type_align(element_type, types)?;
            let stride = align_up(elem_size, elem_align);
            for (i, el) in elements.iter().enumerate() {
                emit_const_init(out, &sub_addr(i as u64 * stride), el, element_type, types, indent)?;
            }
        }
        Constant::Struct { values, is_packed, .. } => {
            let resolved = resolve_named(ty.clone(), types)?;
            let elem_types = match resolved.as_ref() {
                Type::StructType { element_types, .. } => element_types.clone(),
                other => bail!("struct initializer for non-struct type: {other}"),
            };
            let layout = struct_layout(&elem_types, *is_packed, types)?;
            for (i, val) in values.iter().enumerate() {
                emit_const_init(out, &sub_addr(layout.offsets[i]), val, &elem_types[i], types, indent)?;
            }
        }
        Constant::Float(f) => match ty.as_ref() {
            Type::FPType(FPType::Single) => {
                let _ = writeln!(out, "{indent}_store_f32({addr}, {})", float_literal(f));
            }
            Type::FPType(FPType::Double) => {
                let _ = writeln!(out, "{indent}_store_f64({addr}, {})", float_literal(f));
            }
            other => bail!("Float initializer at non-FP type {other}"),
        },
        Constant::GlobalReference { name, .. } => {
            let _ = writeln!(out, "{indent}_store({addr}, {}, 64)", global_to_var(name));
        }
        // Constexpr expressions (GEP / inttoptr / add / etc.) appear in
        // C++ typeinfo vtable+name fields. We don't model them — leave the
        // bytes as MEM's implicit zero and rely on the codepath that uses
        // these globals (currently only RTTI parent-chain walks via field
        // 2) being separately wired up.
        other => {
            let _ = writeln!(
                out,
                "{indent}# skip unsupported constant initializer: {}",
                other.to_string().replace('\n', " ")
            );
        }
    }
    Ok(())
}

fn emit_function(out: &mut String, func: &Function, types: &Types) -> Result<()> {
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
        emit_multi_block(out, func, types)?;
    } else {
        let bb = &func.basic_blocks[0];
        for instr in &bb.instrs {
            emit_instruction(out, instr, "    ", types)?;
        }
        emit_terminator(out, &bb.term, &bb.name, func, "    ")?;
    }

    let _ = writeln!(out, "}}");
    let _ = writeln!(out);
    Ok(())
}

fn emit_multi_block(out: &mut String, func: &Function, types: &Types) -> Result<()> {
    let entry = &func.basic_blocks[0];
    let _ = writeln!(out, "    block = \"{}\"", block_label(&entry.name));
    let _ = writeln!(out, "    while (1) {{");
    for (i, bb) in func.basic_blocks.iter().enumerate() {
        let kw = if i == 0 { "if" } else { "else if" };
        let _ = writeln!(out, "        {kw} (block == \"{}\") {{", block_label(&bb.name));
        for instr in &bb.instrs {
            emit_instruction(out, instr, "            ", types)?;
        }
        emit_terminator(out, &bb.term, &bb.name, func, "            ")?;
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
    types: &Types,
) -> Result<()> {
    use Instruction::*;
    match instr {
        Add(i) => binop(out, indent, &i.dest, &i.operand0, "+", &i.operand1),
        Sub(i) => binop(out, indent, &i.dest, &i.operand0, "-", &i.operand1),
        Mul(i) => binop(out, indent, &i.dest, &i.operand0, "*", &i.operand1),
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
        Call(call) => emit_call(out, call, indent),
        Alloca(a) => {
            let elem_size = type_size_bytes(&a.allocated_type, types)?;
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
            let val_ty = s.value.get_type(types);
            emit_store_at(out, indent, &addr, &val, &val_ty)
        }
        ExtractValue(ev) => emit_extractvalue(out, ev, indent, types),
        InsertValue(iv) => emit_insertvalue(out, iv, indent, types),
        AtomicRMW(a) => emit_atomicrmw(out, a, indent, types),
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
            let size = type_size_bytes(&lp.result_type, types)?;
            let _ = writeln!(out, "{indent}{dest} = _alloc({size})");
            let _ = writeln!(out, "{indent}_store({dest}, EXC_OBJ, 64)");
            let _ = writeln!(out, "{indent}_store({dest} + 8, EXC_TYPE_ID, 32)");
            let _ = writeln!(out, "{indent}UNWINDING = 0");
            Ok(())
        }
        GetElementPtr(g) => emit_gep(out, g, indent, types),
        BitCast(c) => {
            let dest = name_to_var(&c.dest);
            let val = operand_str(&c.operand);
            let src_ty = c.operand.get_type(types);
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

fn emit_call(out: &mut String, call: &llvm_ir::instruction::Call, indent: &str) -> Result<()> {
    let target_name = match &call.function {
        Either::Right(Operand::ConstantOperand(c)) => match c.as_ref() {
            Constant::GlobalReference { name, .. } => match name {
                Name::Name(s) => s.to_string(),
                Name::Number(n) => n.to_string(),
            },
            _ => bail!("indirect call target is not a global reference"),
        },
        Either::Right(op) => return emit_indirect_call(out, call, op, indent),
        Either::Left(_) => bail!("inline assembly is not supported"),
    };

    if let Some(rest) = target_name.strip_prefix("llvm.") {
        return emit_intrinsic(out, call, indent, &target_name, rest);
    }

    let args: Vec<String> = call.arguments.iter().map(|(op, _)| operand_str(op)).collect();
    if emit_libc(out, &target_name, &args, call.dest.as_ref(), indent)? {
        return Ok(());
    }

    let target = format!("fn_{}", sanitize(&target_name));
    let call_expr = format!("{target}({})", args.join(", "));

    match &call.dest {
        Some(dest) => {
            let _ = writeln!(out, "{indent}{} = {call_expr}", name_to_var(dest));
        }
        None => {
            let _ = writeln!(out, "{indent}{call_expr}");
        }
    }
    // Propagate exceptions: a regular call to a user function that throws
    // sets UNWINDING; we must abandon the rest of this function so the
    // caller (or its enclosing landingpad) can react.
    let _ = writeln!(out, "{indent}if (UNWINDING) return");
    Ok(())
}

fn emit_indirect_call(
    out: &mut String,
    call: &llvm_ir::instruction::Call,
    fp_op: &Operand,
    indent: &str,
) -> Result<()> {
    let fp = operand_str(fp_op);
    let args: Vec<String> = call.arguments.iter().map(|(op, _)| operand_str(op)).collect();
    if args.len() > MAX_ICALL_ARITY {
        bail!(
            "indirect call has {} args; dispatcher only supports up to {}",
            args.len(),
            MAX_ICALL_ARITY
        );
    }
    let mut all = vec![fp];
    all.extend(args);
    let call_expr = format!("_icall({})", all.join(", "));
    match &call.dest {
        Some(d) => {
            let _ = writeln!(out, "{indent}{} = {call_expr}", name_to_var(d));
        }
        None => {
            let _ = writeln!(out, "{indent}{call_expr}");
        }
    }
    let _ = writeln!(out, "{indent}if (UNWINDING) return");
    Ok(())
}

// Maps stdlib calls onto runtime helpers / gawk built-ins. Returns true when
// the call was handled.
fn emit_libc(
    out: &mut String,
    name: &str,
    args: &[String],
    dest: Option<&Name>,
    indent: &str,
) -> Result<bool> {
    let assign = |expr: String, out: &mut String| match dest {
        Some(d) => {
            let _ = writeln!(out, "{indent}{} = {expr}", name_to_var(d));
        }
        None => {
            let _ = writeln!(out, "{indent}{expr}");
        }
    };
    match name {
        "puts" => {
            let _ = writeln!(out, "{indent}print _cstr({})", args[0]);
            // puts returns non-negative on success; pin to 0.
            if let Some(d) = dest {
                let _ = writeln!(out, "{indent}{} = 0", name_to_var(d));
            }
        }
        "putchar" => {
            let _ = writeln!(out, "{indent}printf \"%c\", {}", args[0]);
            if let Some(d) = dest {
                let _ = writeln!(out, "{indent}{} = {}", name_to_var(d), args[0]);
            }
        }
        "printf" => emit_printf(out, args, dest, indent),
        "malloc" => assign(format!("_alloc({})", args[0]), out),
        "free" => {
            // No-op: bump allocator never reclaims.
        }
        "exit" => {
            let _ = writeln!(out, "{indent}exit {}", args[0]);
        }
        "abort" => {
            let _ = writeln!(out, "{indent}exit 134");
        }
        "memcpy" => assign(format!("_memcpy({}, {}, {})", args[0], args[1], args[2]), out),
        "memmove" => assign(format!("_memmove({}, {}, {})", args[0], args[1], args[2]), out),
        "memset" => assign(format!("_memset({}, {}, {})", args[0], args[1], args[2]), out),
        "strlen" => assign(format!("_strlen({})", args[0]), out),
        "strcmp" => assign(format!("_strcmp({}, {})", args[0], args[1]), out),
        // strtol/strtod ignore base/end-pointer for now: the dominant use is
        // strtol(s, nullptr, 10) and strtod(s, nullptr).
        "atoi" | "atol" | "atoll" | "strtol" | "strtoll" | "strtoul" | "strtoull" => {
            assign(format!("_atoi({})", args[0]), out);
        }
        "atof" | "strtod" | "strtof" | "strtold" => {
            assign(format!("_atof({})", args[0]), out);
        }
        "memset_pattern4" => {
            let _ = writeln!(out, "{indent}_memset_pattern({}, {}, {}, 4)", args[0], args[1], args[2]);
        }
        "memset_pattern8" => {
            let _ = writeln!(out, "{indent}_memset_pattern({}, {}, {}, 8)", args[0], args[1], args[2]);
        }
        "memset_pattern16" => {
            let _ = writeln!(out, "{indent}_memset_pattern({}, {}, {}, 16)", args[0], args[1], args[2]);
        }
        // C++ ABI exception runtime. Stack unwinding doesn't exist here;
        // we model it with a global UNWINDING flag, EXC_OBJ, and EXC_TYPE_ID
        // (the typeinfo address, which we use as the type id).
        "__cxa_allocate_exception" => assign(format!("_alloc({})", args[0]), out),
        "__cxa_throw" => {
            // (obj, typeinfo, dtor) — drop the destructor, set globals, return.
            // The IR emits `unreachable` after this; the early return here
            // makes that explicit.
            let _ = writeln!(out, "{indent}EXC_OBJ = {}", args[0]);
            let _ = writeln!(out, "{indent}EXC_TYPE_ID = {}", args[1]);
            let _ = writeln!(out, "{indent}UNWINDING = 1");
            let _ = writeln!(out, "{indent}return");
        }
        "__cxa_begin_catch" => assign(args[0].clone(), out),
        "__cxa_end_catch" => {}
        "__cxa_rethrow" => {
            let _ = writeln!(out, "{indent}UNWINDING = 1");
            let _ = writeln!(out, "{indent}return");
        }
        // C++ operator new / delete (and the nothrow / sized variants).
        // Mangled names per Itanium ABI.
        "_Znwm" | "_Znam" | "_ZnwmRKSt9nothrow_t" | "_ZnamRKSt9nothrow_t" => {
            assign(format!("_alloc({})", args[0]), out);
        }
        "_ZdlPv" | "_ZdaPv" | "_ZdlPvm" | "_ZdaPvm" | "_ZdlPvRKSt9nothrow_t"
        | "_ZdaPvRKSt9nothrow_t" => {
            // No-op: bump allocator never reclaims.
        }
        _ => return Ok(false),
    }
    Ok(true)
}

fn emit_printf(out: &mut String, args: &[String], dest: Option<&Name>, indent: &str) {
    let _ = writeln!(out, "{indent}delete _PA");
    for (i, arg) in args.iter().skip(1).enumerate() {
        let _ = writeln!(out, "{indent}_PA[{i}] = {arg}");
    }
    match dest {
        Some(d) => {
            let _ = writeln!(out, "{indent}{} = _printf({})", name_to_var(d), args[0]);
        }
        None => {
            let _ = writeln!(out, "{indent}_printf({})", args[0]);
        }
    }
}

fn emit_intrinsic(
    out: &mut String,
    call: &llvm_ir::instruction::Call,
    indent: &str,
    full_name: &str,
    rest: &str,
) -> Result<()> {
    let base = rest.split('.').next().unwrap_or(rest);
    let args: Vec<String> = call.arguments.iter().map(|(op, _)| operand_str(op)).collect();
    let dest = call.dest.as_ref();

    let assign = |expr: String, out: &mut String| match dest {
        Some(d) => {
            let _ = writeln!(out, "{indent}{} = {expr}", name_to_var(d));
        }
        None => {
            let _ = writeln!(out, "{indent}{expr}");
        }
    };

    match base {
        "smax" | "umax" if args.len() == 2 => {
            assign(format!("{a} > {b} ? {a} : {b}", a = args[0], b = args[1]), out);
        }
        "smin" | "umin" if args.len() == 2 => {
            assign(format!("{a} < {b} ? {a} : {b}", a = args[0], b = args[1]), out);
        }
        // llvm.abs takes (value, is_int_min_poison). We ignore the poison flag.
        "abs" if !args.is_empty() => {
            assign(format!("{a} < 0 ? -{a} : {a}", a = args[0]), out);
        }
        // (dst, src, len, is_volatile) — drop the volatile flag.
        "memcpy" if args.len() >= 3 => {
            let _ = writeln!(out, "{indent}_memcpy({}, {}, {})", args[0], args[1], args[2]);
        }
        "memmove" if args.len() >= 3 => {
            let _ = writeln!(out, "{indent}_memmove({}, {}, {})", args[0], args[1], args[2]);
        }
        // (dst, val, len, is_volatile)
        "memset" if args.len() >= 3 => {
            let _ = writeln!(out, "{indent}_memset({}, {}, {})", args[0], args[1], args[2]);
        }
        // Fused multiply-add (a * b + c). Float since gawk is double; for ints
        // it shows up identically. We don't get IEEE 754 fma rounding semantics.
        "fmuladd" | "fma" if args.len() == 3 => {
            assign(format!("{} * {} + {}", args[0], args[1], args[2]), out);
        }
        "sqrt" if args.len() == 1 => {
            assign(format!("sqrt({})", args[0]), out);
        }
        "sin" if args.len() == 1 => assign(format!("sin({})", args[0]), out),
        "cos" if args.len() == 1 => assign(format!("cos({})", args[0]), out),
        "tan" if args.len() == 1 => {
            assign(format!("(sin({a}) / cos({a}))", a = args[0]), out);
        }
        "exp" if args.len() == 1 => assign(format!("exp({})", args[0]), out),
        "log" if args.len() == 1 => assign(format!("log({})", args[0]), out),
        "pow" if args.len() == 2 => assign(format!("({} ^ {})", args[0], args[1]), out),
        "fabs" if args.len() == 1 => {
            assign(format!("({a} < 0 ? -{a} : {a})", a = args[0]), out);
        }
        "floor" if args.len() == 1 => {
            assign(format!("({a} >= 0 ? int({a}) : -int(-{a} + (-{a} > int(-{a}))))", a = args[0]), out);
        }
        "ceil" if args.len() == 1 => {
            assign(format!("({a} >= 0 ? int({a}) + ({a} > int({a})) : -int(-{a}))", a = args[0]), out);
        }
        "trunc" if args.len() == 1 => {
            assign(format!("({a} >= 0 ? int({a}) : -int(-{a}))", a = args[0]), out);
        }
        // Catch matching honours the typeinfo parent chain (single
        // inheritance) instead of identity, so `catch (Base&)` accepts
        // a `Derived` throw.
        "eh" if args.len() == 1 => {
            assign(format!("_typeid_for({})", args[0]), out);
        }
        // Pure markers for the optimizer; nothing to emit at runtime.
        "lifetime" | "dbg" | "assume" | "experimental" => {}
        _ => bail!("intrinsic `{full_name}` is not implemented yet"),
    }
    Ok(())
}

fn emit_terminator(
    out: &mut String,
    term: &Terminator,
    current_block: &Name,
    func: &Function,
    indent: &str,
) -> Result<()> {
    use Terminator::*;
    match term {
        Ret(r) => {
            match &r.return_operand {
                Some(op) => {
                    let _ = writeln!(out, "{indent}return {}", operand_str(op));
                }
                None => {
                    let _ = writeln!(out, "{indent}return");
                }
            }
            Ok(())
        }
        Br(b) => emit_branch(out, indent, &b.dest, current_block, func),
        CondBr(b) => {
            let cond = operand_str(&b.condition);
            let _ = writeln!(out, "{indent}if ({cond}) {{");
            let inner = format!("{indent}    ");
            emit_branch(out, &inner, &b.true_dest, current_block, func)?;
            let _ = writeln!(out, "{indent}}} else {{");
            emit_branch(out, &inner, &b.false_dest, current_block, func)?;
            let _ = writeln!(out, "{indent}}}");
            Ok(())
        }
        Switch(s) => {
            let val = operand_str(&s.operand);
            let inner = format!("{indent}    ");
            if s.dests.is_empty() {
                emit_branch(out, indent, &s.default_dest, current_block, func)?;
            } else {
                for (i, (c, target)) in s.dests.iter().enumerate() {
                    let kw = if i == 0 { "if" } else { "else if" };
                    let _ = writeln!(out, "{indent}{kw} ({val} == {}) {{", constant_str(c));
                    emit_branch(out, &inner, target, current_block, func)?;
                    let _ = writeln!(out, "{indent}}}");
                }
                let _ = writeln!(out, "{indent}else {{");
                emit_branch(out, &inner, &s.default_dest, current_block, func)?;
                let _ = writeln!(out, "{indent}}}");
            }
            Ok(())
        }
        Invoke(inv) => emit_invoke(out, inv, current_block, func, indent),
        Resume(r) => {
            let addr = operand_str(&r.operand);
            let _ = writeln!(out, "{indent}EXC_OBJ = _load({addr}, 64)");
            let _ = writeln!(out, "{indent}EXC_TYPE_ID = _load({addr} + 8, 32)");
            let _ = writeln!(out, "{indent}UNWINDING = 1");
            let _ = writeln!(out, "{indent}return");
            Ok(())
        }
        Unreachable(_) => {
            // No semantics, but the block must end with a return so the
            // multi-block dispatcher loop exits.
            let _ = writeln!(out, "{indent}return");
            Ok(())
        }
        other => bail!("terminator not implemented: {other}"),
    }
}

fn emit_invoke(
    out: &mut String,
    inv: &llvm_ir::terminator::Invoke,
    current_block: &Name,
    func: &Function,
    indent: &str,
) -> Result<()> {
    let args: Vec<String> = inv.arguments.iter().map(|(op, _)| operand_str(op)).collect();
    let result = name_to_var(&inv.result);

    match &inv.function {
        Either::Right(Operand::ConstantOperand(c)) => {
            let target_name = match c.as_ref() {
                Constant::GlobalReference { name, .. } => match name {
                    Name::Name(s) => s.to_string(),
                    Name::Number(n) => n.to_string(),
                },
                _ => bail!("invoke target is not a global reference"),
            };
            if !emit_libc(out, &target_name, &args, Some(&inv.result), indent)? {
                let target = format!("fn_{}", sanitize(&target_name));
                let _ = writeln!(out, "{indent}{result} = {target}({})", args.join(", "));
            }
        }
        Either::Right(op) => {
            // Indirect invoke: dispatch through _icall, then check UNWINDING
            // the same way as a direct invoke.
            if args.len() > MAX_ICALL_ARITY {
                bail!(
                    "indirect invoke has {} args; dispatcher only supports up to {}",
                    args.len(),
                    MAX_ICALL_ARITY
                );
            }
            let fp = operand_str(op);
            let mut all = vec![fp];
            all.extend(args);
            let _ = writeln!(out, "{indent}{result} = _icall({})", all.join(", "));
        }
        Either::Left(_) => bail!("inline assembly invoke not supported"),
    }

    let _ = writeln!(out, "{indent}if (UNWINDING) {{");
    let inner = format!("{indent}    ");
    emit_branch(out, &inner, &inv.exception_label, current_block, func)?;
    let _ = writeln!(out, "{indent}}} else {{");
    emit_branch(out, &inner, &inv.return_label, current_block, func)?;
    let _ = writeln!(out, "{indent}}}");
    Ok(())
}

fn emit_branch(
    out: &mut String,
    indent: &str,
    target: &Name,
    current_block: &Name,
    func: &Function,
) -> Result<()> {
    let target_bb = func
        .basic_blocks
        .iter()
        .find(|b| &b.name == target)
        .ok_or_else(|| anyhow!("branch target `{target}` not found in function `{}`", func.name))?;

    // Resolve phis sequentially. Correct for the common case where phi
    // destinations don't feed each other in the same block; the swap/cycle
    // case is left as a future temp-based parallel copy.
    for instr in &target_bb.instrs {
        if let Instruction::Phi(phi) = instr {
            let (val, _) = phi
                .incoming_values
                .iter()
                .find(|(_, src)| src == current_block)
                .ok_or_else(|| {
                    anyhow!(
                        "phi `{}` has no incoming value from `{current_block}` in `{}`",
                        phi.dest,
                        func.name
                    )
                })?;
            let _ = writeln!(out, "{indent}{} = {}", name_to_var(&phi.dest), operand_str(val));
        }
    }
    let _ = writeln!(out, "{indent}block = \"{}\"", block_label(target));
    Ok(())
}

fn operand_str(op: &Operand) -> String {
    match op {
        Operand::LocalOperand { name, .. } => name_to_var(name),
        Operand::ConstantOperand(c) => constant_str(c),
        Operand::MetadataOperand => "0".to_string(),
    }
}

fn constant_str(c: &ConstantRef) -> String {
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
        // Fall-through: bake to 0 rather than writing /* comment */ syntax
        // that wouldn't survive being inlined into an awk expression. Whatever
        // codepath consumes this value is on its own; for our smoke fixtures
        // these are typically dead branches.
        _ => "0".to_string(),
    }
}

fn float_literal(f: &Float) -> String {
    // Debug formatting (e.g. "1.0", "3.14") guarantees a decimal point so
    // awk's strtod parses it as a number rather than juxtaposed tokens.
    match f {
        Float::Single(v) => format!("{v:?}"),
        Float::Double(v) => format!("{v:?}"),
        other => format!("0 /* unsupported float: {other} */"),
    }
}

fn sign_extend(value: u64, bits: u32) -> i64 {
    if bits >= 64 {
        return value as i64;
    }
    let shift = 64 - bits;
    ((value as i64) << shift) >> shift
}

fn operand_bits(op: &Operand) -> Result<u32> {
    match op {
        Operand::LocalOperand { ty, .. } => type_bits(ty),
        Operand::ConstantOperand(c) => match c.as_ref() {
            Constant::Int { bits, .. } => Ok(*bits),
            other => bail!("operand has non-integer constant type: {other}"),
        },
        Operand::MetadataOperand => bail!("metadata operand has no bit width"),
    }
}

fn type_bits(ty: &TypeRef) -> Result<u32> {
    match ty.as_ref() {
        Type::IntegerType { bits } => Ok(*bits),
        other => bail!("expected integer type, got {other}"),
    }
}

// Bit width to use when loading/storing a value of this type. Pointers are
// fixed at 64 bits (x86_64 model).
fn mem_bits(ty: &TypeRef) -> Result<u32> {
    match ty.as_ref() {
        Type::IntegerType { bits } => Ok(*bits),
        Type::PointerType { .. } => Ok(64),
        Type::FPType(FPType::Single) => Ok(32),
        Type::FPType(FPType::Double) => Ok(64),
        other => bail!("load/store of type {other} not supported"),
    }
}

fn type_size_bytes(ty: &TypeRef, types: &Types) -> Result<u64> {
    match ty.as_ref() {
        Type::IntegerType { bits } => Ok(((*bits as u64) + 7) / 8),
        Type::PointerType { .. } => Ok(8),
        Type::FPType(FPType::Single) => Ok(4),
        Type::FPType(FPType::Double) => Ok(8),
        Type::ArrayType { element_type, num_elements } => {
            let elem = type_size_bytes(element_type, types)?;
            let align = type_align(element_type, types)?;
            let stride = align_up(elem, align);
            Ok(stride * (*num_elements as u64))
        }
        Type::StructType { element_types, is_packed } => {
            Ok(struct_layout(element_types, *is_packed, types)?.size)
        }
        Type::NamedStructType { name } => match types.named_struct_def(name) {
            Some(NamedStructDef::Defined(def)) => type_size_bytes(def, types),
            _ => bail!("opaque or undefined struct: {name}"),
        },
        other => bail!("size of type {other} not supported"),
    }
}

fn type_align(ty: &TypeRef, types: &Types) -> Result<u64> {
    match ty.as_ref() {
        Type::IntegerType { bits } => Ok(((*bits as u64) + 7) / 8).map(|b| b.max(1)),
        Type::PointerType { .. } => Ok(8),
        Type::FPType(FPType::Single) => Ok(4),
        Type::FPType(FPType::Double) => Ok(8),
        Type::ArrayType { element_type, .. } => type_align(element_type, types),
        Type::StructType { element_types, is_packed } => {
            if *is_packed {
                Ok(1)
            } else {
                let mut max = 1u64;
                for el in element_types {
                    let a = type_align(el, types)?;
                    if a > max {
                        max = a;
                    }
                }
                Ok(max)
            }
        }
        Type::NamedStructType { name } => match types.named_struct_def(name) {
            Some(NamedStructDef::Defined(def)) => type_align(def, types),
            _ => bail!("opaque or undefined struct: {name}"),
        },
        other => bail!("alignment of type {other} not supported"),
    }
}

struct StructLayout {
    size: u64,
    offsets: Vec<u64>,
}

fn struct_layout(elements: &[TypeRef], packed: bool, types: &Types) -> Result<StructLayout> {
    let mut offsets = Vec::with_capacity(elements.len());
    let mut offset = 0u64;
    let mut max_align = 1u64;
    for el in elements {
        let sz = type_size_bytes(el, types)?;
        let al = if packed { 1 } else { type_align(el, types)? };
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

fn align_up(x: u64, a: u64) -> u64 {
    if a <= 1 { x } else { (x + a - 1) / a * a }
}

fn is_aggregate(ty: &TypeRef) -> bool {
    matches!(
        ty.as_ref(),
        Type::StructType { .. } | Type::ArrayType { .. } | Type::NamedStructType { .. }
    )
}

fn is_undef_or_zero(op: &Operand) -> bool {
    if let Operand::ConstantOperand(c) = op {
        matches!(
            c.as_ref(),
            Constant::Undef(_) | Constant::Poison(_) | Constant::AggregateZero(_)
        )
    } else {
        false
    }
}

// Walk an aggregate type with constant indices, returning (byte_offset,
// leaf_type). Used by extractvalue / insertvalue.
fn aggregate_walk(
    base_ty: &TypeRef,
    indices: &[u32],
    types: &Types,
) -> Result<(u64, TypeRef)> {
    let mut current = resolve_named(base_ty.clone(), types)?;
    let mut offset = 0u64;
    for &idx in indices {
        current = resolve_named(current, types)?;
        match current.as_ref() {
            Type::ArrayType { element_type, .. } => {
                let stride = align_up(
                    type_size_bytes(element_type, types)?,
                    type_align(element_type, types)?,
                );
                offset += stride * idx as u64;
                current = element_type.clone();
            }
            Type::StructType { element_types, is_packed } => {
                let layout = struct_layout(element_types, *is_packed, types)?;
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

fn emit_load_at(
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

fn emit_store_at(
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

fn emit_extractvalue(
    out: &mut String,
    ev: &llvm_ir::instruction::ExtractValue,
    indent: &str,
    types: &Types,
) -> Result<()> {
    let agg_ty = ev.aggregate.get_type(types);
    let dest = name_to_var(&ev.dest);
    let src = operand_str(&ev.aggregate);
    let (offset, leaf_ty) = aggregate_walk(&agg_ty, &ev.indices, types)?;
    let field = if offset == 0 { src } else { format!("{src} + {offset}") };
    if is_aggregate(&leaf_ty) {
        let leaf_size = type_size_bytes(&leaf_ty, types)?;
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
fn emit_atomicrmw(
    out: &mut String,
    a: &llvm_ir::instruction::AtomicRMW,
    indent: &str,
    types: &Types,
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

fn emit_insertvalue(
    out: &mut String,
    iv: &llvm_ir::instruction::InsertValue,
    indent: &str,
    types: &Types,
) -> Result<()> {
    let agg_ty = iv.aggregate.get_type(types);
    let size = type_size_bytes(&agg_ty, types)?;
    let dest = name_to_var(&iv.dest);
    let _ = writeln!(out, "{indent}{dest} = _alloc({size})");
    // Skip the memcpy when the source is undef/poison/zero — MEM auto-zeros
    // and the insert below will overwrite the relevant field anyway.
    if !is_undef_or_zero(&iv.aggregate) {
        let src = operand_str(&iv.aggregate);
        let _ = writeln!(out, "{indent}_memcpy({dest}, {src}, {size})");
    }
    let (offset, leaf_ty) = aggregate_walk(&agg_ty, &iv.indices, types)?;
    let field = if offset == 0 { dest.clone() } else { format!("{dest} + {offset}") };
    let val = operand_str(&iv.element);
    let elem_ty = iv.element.get_type(types);
    if is_aggregate(&elem_ty) {
        if !is_undef_or_zero(&iv.element) {
            let elem_size = type_size_bytes(&elem_ty, types)?;
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

fn resolve_named(ty: TypeRef, types: &Types) -> Result<TypeRef> {
    let mut cur = ty;
    while let Type::NamedStructType { name } = cur.as_ref() {
        match types.named_struct_def(name) {
            Some(NamedStructDef::Defined(def)) => cur = def.clone(),
            _ => bail!("opaque or undefined struct: {name}"),
        }
    }
    Ok(cur)
}

fn emit_gep(
    out: &mut String,
    gep: &llvm_ir::instruction::GetElementPtr,
    indent: &str,
    types: &Types,
) -> Result<()> {
    let dest = name_to_var(&gep.dest);
    let mut const_off: i64 = 0;
    let mut runtime: Vec<(u64, String)> = Vec::new();

    let mut current = resolve_named(gep.source_element_type.clone(), types)?;
    let mut idxs = gep.indices.iter();

    // First index strides over copies of source_element_type (treats base as
    // a 1-element array of that type), but does not descend.
    let first = idxs
        .next()
        .ok_or_else(|| anyhow!("GEP must have at least one index"))?;
    let stride = type_size_bytes(&current, types)?;
    accumulate_index(first, stride, &mut const_off, &mut runtime)?;

    // Remaining indices descend into aggregates.
    for idx in idxs {
        current = resolve_named(current, types)?;
        match current.as_ref() {
            Type::ArrayType { element_type, .. } => {
                let stride = type_size_bytes(element_type, types)?;
                accumulate_index(idx, stride, &mut const_off, &mut runtime)?;
                current = element_type.clone();
            }
            Type::StructType { element_types, is_packed } => {
                let layout = struct_layout(element_types, *is_packed, types)?;
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

fn name_to_var(name: &Name) -> String {
    match name {
        Name::Number(n) => format!("r{n}"),
        Name::Name(s) => format!("r_{}", sanitize(s)),
    }
}

fn func_to_var(name: &str) -> String {
    format!("fn_{}", sanitize(name))
}

fn global_to_var(name: &Name) -> String {
    match name {
        Name::Number(n) => format!("g{n}"),
        Name::Name(s) => format!("g_{}", sanitize(s)),
    }
}

fn block_label(name: &Name) -> String {
    match name {
        Name::Number(n) => format!("b{n}"),
        Name::Name(s) => format!("b_{}", sanitize(s)),
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}
