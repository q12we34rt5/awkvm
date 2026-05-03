use std::path::Path;

use anyhow::{Result, anyhow};
use llvm_ir::Module;

pub fn load(path: &Path) -> Result<Module> {
    Module::from_bc_path(path).map_err(|e| anyhow!("{e}"))
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
