use std::collections::HashMap;

use llvm_ir::{Constant, ConstantRef, Module, Name, Type};

// Walk the `@llvm.global.annotations` global and pull out
// `__attribute__((annotate("..."))) ` strings keyed by the annotated
// function's name. The full annotation text is returned verbatim;
// callers filter by prefix (e.g. `awkvm_fn`, `awkvm_export`)
// themselves.
//
// Layout (per Itanium-ABI clang -O1):
//   @llvm.global.annotations = appending global
//       [N x { ptr, ptr, ptr, i32, ptr }]
//       [ {ptr @function, ptr @.str, ptr @.file, i32 line, ptr null}, ... ]
// Field 0 is the annotated value pointer; field 1 is the annotation
// string global. We resolve both, then read the string global's
// `[N x i8]` initializer back into a Rust string.
pub(super) fn collect(module: &Module) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(annot) = module
        .global_vars
        .iter()
        .find(|gv| matches!(&gv.name, Name::Name(s) if s.as_str() == "llvm.global.annotations"))
    else {
        return out;
    };
    let Some(init) = &annot.initializer else {
        return out;
    };
    let Constant::Array { elements, .. } = init.as_ref() else {
        return out;
    };

    for entry in elements {
        let Constant::Struct { values, .. } = entry.as_ref() else {
            continue;
        };
        if values.len() < 2 {
            continue;
        }
        let Some(fn_name) = peel_global_name(&values[0]) else {
            continue;
        };
        let Some(str_name) = peel_global_name(&values[1]) else {
            continue;
        };
        let Some(text) = read_string_constant(module, &str_name) else {
            continue;
        };
        out.insert(fn_name, text);
    }
    out
}

// Recursively unwrap BitCast / GEP wrappers to recover the underlying
// global symbol name. Modern opaque-pointer IR usually stores a direct
// GlobalReference, but constexpr forms (or pre-opaque IR) wrap it.
fn peel_global_name(c: &ConstantRef) -> Option<String> {
    match c.as_ref() {
        Constant::GlobalReference { name, .. } => match name {
            Name::Name(s) => Some(s.to_string()),
            _ => None,
        },
        Constant::BitCast(c) => peel_global_name(&c.operand),
        Constant::GetElementPtr(g) => peel_global_name(&g.address),
        _ => None,
    }
}

// Decode an `[N x i8]` string global's initializer back to a Rust
// String. Stops at the first NUL byte.
fn read_string_constant(module: &Module, name: &str) -> Option<String> {
    let gv = module
        .global_vars
        .iter()
        .find(|gv| matches!(&gv.name, Name::Name(s) if s.as_str() == name))?;
    let init = gv.initializer.as_ref()?;
    let Constant::Array { elements, element_type } = init.as_ref() else {
        return None;
    };
    if !matches!(element_type.as_ref(), Type::IntegerType { bits: 8 }) {
        return None;
    }
    let mut bytes = Vec::with_capacity(elements.len());
    for elem in elements {
        let Constant::Int { value, .. } = elem.as_ref() else {
            return None;
        };
        let b = (*value & 0xFF) as u8;
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8(bytes).ok()
}
