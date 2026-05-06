use std::collections::HashMap;
use std::fmt::Write as _;

use anyhow::{Result, bail};
use llvm_ir::{Constant, ConstantRef, Module, Name, Type, TypeRef, types::{FPType, Typed}};

use super::MAX_ICALL_ARITY;
use super::names::{float_literal, global_to_var, sanitize};
use super::probe_map::STREAM_GLOBALS;
use super::types::{LayoutCx, align_up, resolve_named, sign_extend};

pub(super) fn emit_icall_dispatcher(
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

pub(super) fn emit_globals_init(
    out: &mut String,
    module: &Module,
    fn_ids: &HashMap<String, i64>,
    cx: &mut LayoutCx<'_>,
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
        let size = cx.size(&storage_ty)?;
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
            cx,
            "    ",
        )?;
    }
    // Stream registry. STREAM_GLOBALS maps recognized libc++ global
    // names (cerr, clog, cin, ...) to `<table>=<value>` metadata that
    // selects which awk runtime table the address gets registered in:
    //   dest=...  → _OSTREAM_DEST  (consulted by _ostream_dest)
    //   src=...   → _ISTREAM_SRC   (consulted by _istream_skip_ws)
    // cout is intentionally absent (defaults to stdout via absent-key
    // path); same for any unrecognized ostream subclass.
    for gv in &module.global_vars {
        let Name::Name(name) = &gv.name else { continue };
        let Some((_, meta)) = STREAM_GLOBALS.iter().find(|(m, _)| *m == name.as_str())
        else {
            continue;
        };
        let var = global_to_var(&gv.name);
        if let Some(target) = meta.strip_prefix("dest=") {
            let _ = writeln!(out, "    _OSTREAM_DEST[{var}] = \"{target}\"");
        } else if let Some(source) = meta.strip_prefix("src=") {
            let _ = writeln!(out, "    _ISTREAM_SRC[{var}] = \"{source}\"");
        } else {
            bail!(
                "STREAM_GLOBALS metadata `{meta}` for `{name}` must start with \
                 `dest=` or `src=`"
            );
        }
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
    cx: &mut LayoutCx<'_>,
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
            let elem_size = cx.size(element_type)?;
            let elem_align = cx.align(element_type)?;
            let stride = align_up(elem_size, elem_align);
            for (i, el) in elements.iter().enumerate() {
                emit_const_init(out, &sub_addr(i as u64 * stride), el, element_type, cx, indent)?;
            }
        }
        Constant::Struct { values, is_packed, .. } => {
            let resolved = resolve_named(ty.clone(), cx.types)?;
            let elem_types = match resolved.as_ref() {
                Type::StructType { element_types, .. } => element_types.clone(),
                other => bail!("struct initializer for non-struct type: {other}"),
            };
            let layout = cx.struct_layout(&elem_types, *is_packed)?;
            for (i, val) in values.iter().enumerate() {
                emit_const_init(out, &sub_addr(layout.offsets[i]), val, &elem_types[i], cx, indent)?;
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
