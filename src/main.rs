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

    /// Library mode: skip the `BEGIN { exit fn_main(...) }` boot line so
    /// the emitted script is loadable as a gawk library
    /// (`gawk -f lib.awk -f script.awk`). Pair with
    /// `__attribute__((annotate("awkvm_export")))` on C functions to
    /// expose them under their bare names to external awk callers.
    #[arg(long)]
    library: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    awkvm::compile(&cli.input, cli.output.as_deref(), &cli.link, cli.library)
}
