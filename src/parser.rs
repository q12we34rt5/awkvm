use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use llvm_ir::Module;

pub fn load(path: &Path) -> Result<Module> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("bc") => from_bc(path),
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

fn from_ll(path: &Path) -> Result<Module> {
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
    Ok(module)
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
