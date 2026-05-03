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
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    awkvm::compile(&cli.input, cli.output.as_deref())
}
