use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(name = "awkvm", version, about = "Compile LLVM IR to gawk script")]
struct Cli {
    /// Input LLVM bitcode file (.bc)
    input: PathBuf,

    /// Output awk script path (defaults to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Concatenate awk source from this file into the emitted script.
    /// Functions defined as `function fn_<name>(...)` become callable
    /// from C-side `extern <T> <name>(...)` declarations and from
    /// inline awk via the same `fn_<name>`. May be passed multiple times.
    #[arg(long, value_name = "FILE")]
    link: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    awkvm::compile(&cli.input, cli.output.as_deref(), &cli.link)
}
