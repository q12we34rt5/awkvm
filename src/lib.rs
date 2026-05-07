use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub mod codegen;
pub mod parser;
pub mod runtime;

pub fn compile(
    input: &Path,
    output: Option<&Path>,
    link: &[PathBuf],
    library: bool,
) -> Result<()> {
    let parsed = parser::load(input)
        .with_context(|| format!("failed to parse {}", input.display()))?;

    parser::print_summary(&parsed.module);

    let linked: Vec<String> = link
        .iter()
        .map(|p| {
            fs::read_to_string(p).with_context(|| format!("failed to read {}", p.display()))
        })
        .collect::<Result<_>>()?;

    let awk = codegen::emit(&parsed.module, &parsed.inline_asm, &linked, library)?;
    match output {
        Some(path) => {
            fs::write(path, awk).with_context(|| format!("failed to write {}", path.display()))?
        }
        None => print!("{awk}"),
    }
    Ok(())
}
