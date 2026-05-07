use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub mod codegen;
pub mod parser;
pub mod runtime;

pub fn compile(input: &Path, output: Option<&Path>) -> Result<()> {
    let parsed = parser::load(input)
        .with_context(|| format!("failed to parse {}", input.display()))?;

    parser::print_summary(&parsed.module);

    let awk = codegen::emit(&parsed.module, &parsed.inline_asm)?;
    match output {
        Some(path) => fs::write(path, awk)
            .with_context(|| format!("failed to write {}", path.display()))?,
        None => print!("{awk}"),
    }
    Ok(())
}
