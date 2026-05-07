use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use llvm_ir::Module;

// Module + sidecar info that llvm-ir's IR doesn't surface. Currently
// just inline-asm calls (the LLVM C API hides the asm string and
// constraint string, so we recover them by scanning the .ll text).
pub struct ParsedModule {
    pub module: Module,
    // Inline-asm calls in source order across the whole module:
    // `(asm_string, constraint_string)`. Codegen pops from the front
    // each time an `Either::Left(InlineAssembly)` is encountered.
    pub inline_asm: Vec<(String, String)>,
}

pub fn load(path: &Path) -> Result<ParsedModule> {
    match path.extension().and_then(|e| e.to_str()) {
        // .bc input: no .ll text to scan; inline asm support requires
        // .ll for now. (Workaround: `llvm-dis foo.bc -o foo.ll`.)
        Some("bc") => Ok(ParsedModule {
            module: from_bc(path)?,
            inline_asm: Vec::new(),
        }),
        Some("ll") => from_ll(path),
        Some(other) => bail!(
            "unsupported input extension `.{other}`; expected `.bc` or `.ll`"
        ),
        None => bail!("input has no extension; expected `.bc` or `.ll`"),
    }
}

fn from_bc(path: &Path) -> Result<Module> {
    Module::from_bc_path(path).map_err(|e| anyhow!("{e}"))
}

fn from_ll(path: &Path) -> Result<ParsedModule> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let inline_asm = scan_inline_asm(&text);

    let tmp = tempfile::Builder::new()
        .prefix("awkvm-")
        .tempdir()
        .context("failed to create temp dir for .ll → .bc conversion")?;
    let bc_path = tmp.path().join("input.bc");

    let llvm_as = llvm_as_path();
    let status = Command::new(&llvm_as)
        .arg(path)
        .arg("-o")
        .arg(&bc_path)
        .status()
        .with_context(|| format!("failed to invoke `{}`", llvm_as.display()))?;
    if !status.success() {
        bail!("`{}` exited with {status}", llvm_as.display());
    }

    let mut module = from_bc(&bc_path)?;
    module.name = path.display().to_string();
    Ok(ParsedModule { module, inline_asm })
}

// Recover inline-asm string + constraint string from a .ll text file.
// LLVM's C API doesn't expose these on the InlineAsm value, so the
// llvm-ir crate sees empty placeholders; we reconstruct them by
// pattern-matching the textual IR.
//
// Pattern: the relevant lines look like
//     %3 = tail call i32 asm "AWKVM:$0 = $1 * 2", "=r,r"(i32 21) ...
// or
//     tail call void asm sideeffect "AWKVM:print", ""()
// We walk linearly, find ` asm `, skip the optional flag tokens
// (sideeffect / alignstack / inteldialect), then read two
// double-quoted strings. Quotes inside the asm body are escaped as
// `\NN` hex by the IR printer, so a naïve `find('"')` correctly
// stops at the actual closing quote.
fn scan_inline_asm(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(idx) = line.find(" asm ") else { continue };
        let mut rest = line[idx + " asm ".len()..].trim_start();
        for flag in ["sideeffect ", "alignstack ", "inteldialect "] {
            if let Some(stripped) = rest.strip_prefix(flag) {
                rest = stripped.trim_start();
            }
        }
        let Some(rest1) = rest.strip_prefix('"') else { continue };
        let Some(end_a) = rest1.find('"') else { continue };
        let asm_str = &rest1[..end_a];
        let after = rest1[end_a + 1..].trim_start_matches(|c: char| c == ',' || c.is_whitespace());
        let Some(rest2) = after.strip_prefix('"') else { continue };
        let Some(end_c) = rest2.find('"') else { continue };
        let constraints = &rest2[..end_c];
        out.push((asm_str.to_string(), constraints.to_string()));
    }
    out
}

fn llvm_as_path() -> PathBuf {
    if let Some(prefix) = option_env!("LLVM_SYS_191_PREFIX") {
        let candidate = Path::new(prefix).join("bin").join("llvm-as");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from("llvm-as")
}

pub fn print_summary(module: &Module) {
    eprintln!("module: {}", module.name);
    eprintln!("  source:    {}", module.source_file_name);
    eprintln!(
        "  triple:    {}",
        module.target_triple.as_deref().unwrap_or("(none)")
    );
    eprintln!("  functions: {}", module.functions.len());
    for func in &module.functions {
        eprintln!(
            "    - {} (params={}, blocks={})",
            func.name,
            func.parameters.len(),
            func.basic_blocks.len()
        );
    }
}
