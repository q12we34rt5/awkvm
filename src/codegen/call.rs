use std::fmt::Write as _;

use anyhow::{Result, anyhow, bail};
use either::Either;
use llvm_ir::{Constant, Function, Name, Operand, Terminator};

use super::MAX_ICALL_ARITY;
use super::names::{block_label, constant_str, func_to_var, name_to_var, operand_str};
use super::probe_map::PROBE_MAP;
use super::types::LayoutCx;

pub(super) fn emit_call(
    out: &mut String,
    call: &llvm_ir::instruction::Call,
    indent: &str,
    cx: &mut LayoutCx<'_>,
) -> Result<()> {
    let target_name = match &call.function {
        Either::Right(Operand::ConstantOperand(c)) => match c.as_ref() {
            Constant::GlobalReference { name, .. } => match name {
                Name::Name(s) => s.to_string(),
                Name::Number(n) => n.to_string(),
            },
            _ => bail!("indirect call target is not a global reference"),
        },
        Either::Right(op) => return emit_indirect_call(out, call, op, indent),
        Either::Left(_) => return emit_inline_awk(out, call, indent, cx),
    };

    if let Some(rest) = target_name.strip_prefix("llvm.") {
        return emit_intrinsic(out, call, indent, &target_name, rest);
    }

    let args: Vec<String> = call.arguments.iter().map(|(op, _)| operand_str(op)).collect();
    // printf-family helpers can't be plain awk functions: their variadic
    // args ride in the global _PA[] array, populated inline before the
    // call. Everything else routes through fn_<name> like a user function
    // and is provided by runtime/libc.awk (or by the user's own
    // definition, when shadowing).
    if target_name == "printf" {
        emit_printf(out, &args, call.dest.as_ref(), indent);
        return Ok(());
    }
    if target_name == "fprintf" {
        emit_fprintf(out, &args, call.dest.as_ref(), indent);
        return Ok(());
    }
    if target_name == "scanf" {
        emit_scanf(out, &args, call.dest.as_ref(), indent);
        return Ok(());
    }
    if target_name == "fscanf" {
        emit_fscanf(out, &args, call.dest.as_ref(), indent);
        return Ok(());
    }
    if target_name == "sscanf" {
        emit_sscanf(out, &args, call.dest.as_ref(), indent);
        return Ok(());
    }

    // probe map: mangled libc++ symbols recognized by build.rs get rewritten
    // to a precomputed awk template, with arg0..argN substituted by operand
    // strings. Recognized helpers (currently iostream operators) don't throw,
    // so we skip the UNWINDING check that follows a normal call.
    if let Some(template) = probe_template(&target_name) {
        emit_probe(out, template, &args, call.dest.as_ref(), indent);
        return Ok(());
    }

    let target = func_to_var(&target_name);
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

// Inline assembly with `AWKVM:` prefix → emit raw awk. C source uses
// `__asm__("AWKVM:%0 = %1 * 2" : "=r"(y) : "r"(x));`; clang lowers the
// `%N` operand placeholders to `$N` in the IR's asm string. We strip
// the `AWKVM:` prefix, substitute `$N` → operand string (output operand
// → call dest, then input operands in order), and emit the body as awk
// lines (one per `\n` in the asm string).
//
// llvm-ir 0.11 doesn't expose the asm string and constraints (LLVM C
// API limitation), so parser::scan_inline_asm recovers them from the
// .ll text. The recovered queue is on LayoutCx; we pop one entry here
// per Either::Left site (source order matches by construction).
//
// Lets users escape into the full gawk surface — `system()`, `match()`,
// `mktime()`, bidirectional `|&` — without us adding a runtime intercept
// for every gawk-only feature.
fn emit_inline_awk(
    out: &mut String,
    call: &llvm_ir::instruction::Call,
    indent: &str,
    cx: &mut LayoutCx<'_>,
) -> Result<()> {
    let (assembly, constraints) = cx.next_inline_asm().ok_or_else(|| {
        anyhow!(
            "inline assembly site has no matching entry in the asm queue \
             (was the input a `.bc`? inline asm requires `.ll` input today)"
        )
    })?;
    let body = assembly.strip_prefix("AWKVM:").ok_or_else(|| {
        anyhow!(
            "inline assembly `\"{assembly}\"` lacks the `AWKVM:` prefix; \
             only awkvm-targeted asm is recognized"
        )
    })?;

    // Constraint string: `=r,=r,r,r,~{cc}` — outputs first (prefixed
    // with `=` or `+`), then inputs. We count outputs to know which
    // `$N` indices map to the call's dest vs the call's args.
    let n_outputs = constraints
        .split(',')
        .map(|c| c.trim())
        .take_while(|c| c.starts_with('=') || c.starts_with('+'))
        .count();

    if n_outputs > 1 {
        bail!(
            "inline awk with {n_outputs} output operands is not supported \
             (single output max for now)"
        );
    }

    let dest = call.dest.as_ref().map(name_to_var);
    let inputs: Vec<String> = call
        .arguments
        .iter()
        .map(|(op, _)| operand_str(op))
        .collect();

    // `$$` in the IR asm template is the LLVM-mangled escape for a
    // literal `$` — it stops `$N` operand recognition. Stash those
    // as a sentinel, do the `$N` substitution against the rest,
    // then bring the sentinel back as `$`. NUL is safe because gawk
    // never has it in source.
    let mut substituted = body.replace("$$", "\0");

    // Descending so `$10` is replaced before `$1` etc.
    let total = n_outputs + inputs.len();
    for i in (0..total).rev() {
        let placeholder = format!("${i}");
        let replacement = if i < n_outputs {
            dest.as_deref().unwrap_or("0").to_string()
        } else {
            inputs[i - n_outputs].clone()
        };
        substituted = substituted.replace(&placeholder, &replacement);
    }
    substituted = substituted.replace('\0', "$");

    for line in substituted.lines() {
        let _ = writeln!(out, "{indent}{line}");
    }
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

fn probe_template(name: &str) -> Option<&'static str> {
    PROBE_MAP
        .iter()
        .find_map(|(m, t)| if *m == name { Some(*t) } else { None })
}

fn emit_probe(
    out: &mut String,
    template: &str,
    args: &[String],
    dest: Option<&Name>,
    indent: &str,
) {
    // Descending index so "arg10" is substituted before "arg1" etc.
    let mut expr = template.to_string();
    for (i, arg) in args.iter().enumerate().rev() {
        expr = expr.replace(&format!("arg{i}"), arg);
    }
    match dest {
        Some(d) => {
            let _ = writeln!(out, "{indent}{} = {expr}", name_to_var(d));
        }
        None => {
            let _ = writeln!(out, "{indent}{expr}");
        }
    }
}

fn emit_scanf(out: &mut String, args: &[String], dest: Option<&Name>, indent: &str) {
    // args[0] = fmt, args[1..] = destination pointers.
    let _ = writeln!(out, "{indent}delete _PA");
    for (i, arg) in args.iter().skip(1).enumerate() {
        let _ = writeln!(out, "{indent}_PA[{i}] = {arg}");
    }
    match dest {
        Some(d) => {
            let _ = writeln!(out, "{indent}{} = _scanf({})", name_to_var(d), args[0]);
        }
        None => {
            let _ = writeln!(out, "{indent}_scanf({})", args[0]);
        }
    }
}

fn emit_fscanf(out: &mut String, args: &[String], dest: Option<&Name>, indent: &str) {
    // args[0] = stream (FILE*), args[1] = fmt, args[2..] = destination pointers.
    let _ = writeln!(out, "{indent}delete _PA");
    for (i, arg) in args.iter().skip(2).enumerate() {
        let _ = writeln!(out, "{indent}_PA[{i}] = {arg}");
    }
    match dest {
        Some(d) => {
            let _ = writeln!(
                out,
                "{indent}{} = _fscanf({}, {})",
                name_to_var(d),
                args[0],
                args[1]
            );
        }
        None => {
            let _ = writeln!(out, "{indent}_fscanf({}, {})", args[0], args[1]);
        }
    }
}

fn emit_sscanf(out: &mut String, args: &[String], dest: Option<&Name>, indent: &str) {
    // args[0] = source string addr, args[1] = fmt, args[2..] = destination pointers.
    let _ = writeln!(out, "{indent}delete _PA");
    for (i, arg) in args.iter().skip(2).enumerate() {
        let _ = writeln!(out, "{indent}_PA[{i}] = {arg}");
    }
    match dest {
        Some(d) => {
            let _ = writeln!(
                out,
                "{indent}{} = _sscanf({}, {})",
                name_to_var(d),
                args[0],
                args[1]
            );
        }
        None => {
            let _ = writeln!(out, "{indent}_sscanf({}, {})", args[0], args[1]);
        }
    }
}

fn emit_fprintf(out: &mut String, args: &[String], dest: Option<&Name>, indent: &str) {
    // args[0] = stream (FILE*), args[1] = fmt, args[2..] = varargs.
    let _ = writeln!(out, "{indent}delete _PA");
    for (i, arg) in args.iter().skip(2).enumerate() {
        let _ = writeln!(out, "{indent}_PA[{i}] = {arg}");
    }
    match dest {
        Some(d) => {
            let _ = writeln!(
                out,
                "{indent}{} = _fprintf({}, {})",
                name_to_var(d),
                args[0],
                args[1]
            );
        }
        None => {
            let _ = writeln!(out, "{indent}_fprintf({}, {})", args[0], args[1]);
        }
    }
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

pub(super) fn emit_terminator(
    out: &mut String,
    term: &Terminator,
    current_block: &Name,
    func: &Function,
    indent: &str,
    cx: &mut LayoutCx<'_>,
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
        Invoke(inv) => emit_invoke(out, inv, current_block, func, indent, cx),
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
    _cx: &mut LayoutCx<'_>,
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
            if target_name == "printf" {
                emit_printf(out, &args, Some(&inv.result), indent);
            } else if target_name == "fprintf" {
                emit_fprintf(out, &args, Some(&inv.result), indent);
            } else if target_name == "scanf" {
                emit_scanf(out, &args, Some(&inv.result), indent);
            } else if target_name == "fscanf" {
                emit_fscanf(out, &args, Some(&inv.result), indent);
            } else if target_name == "sscanf" {
                emit_sscanf(out, &args, Some(&inv.result), indent);
            } else if let Some(template) = probe_template(&target_name) {
                emit_probe(out, template, &args, Some(&inv.result), indent);
            } else {
                let target = func_to_var(&target_name);
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
        if let llvm_ir::Instruction::Phi(phi) = instr {
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
