// End-to-end regression test for examples/. Each fixture is compiled .c/.cpp
// → .ll via clang, then .ll → .awk via awkvm, then run through gawk; the
// resulting (exit code, stdout) is compared against a hardcoded expectation.
//
// Expected values come from a one-time capture against a known-good build
// of awkvm. They change when you intentionally change a fixture's behavior;
// otherwise this catches accidental regressions in codegen / runtime.
//
// Requires clang/clang++ from LLVM 19 and gawk in PATH. The clang path is
// resolved through LLVM_SYS_191_PREFIX (matching .cargo/config.toml).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn awkvm_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_awkvm"))
}

fn llvm_bin(name: &str) -> PathBuf {
    let prefix = option_env!("LLVM_SYS_191_PREFIX").unwrap_or("/opt/homebrew/opt/llvm@19");
    PathBuf::from(prefix).join("bin").join(name)
}

struct Outcome {
    exit: i32,
    stdout: Vec<u8>,
}

fn run_fixture(stem: &str, ext: &str, args: &[&str]) -> Outcome {
    let src = manifest_dir().join("examples").join(format!("{stem}.{ext}"));
    assert!(src.exists(), "fixture missing: {}", src.display());

    let tmp = TempDir::new().expect("tempdir");
    let ll = tmp.path().join(format!("{stem}.ll"));
    let awk = tmp.path().join(format!("{stem}.awk"));

    compile_to_ll(&src, ext == "cpp", &ll);
    awkvm_emit(&ll, &awk);
    run_gawk(&awk, args)
}

fn compile_to_ll(src: &Path, cpp: bool, out: &Path) {
    let cc = if cpp { llvm_bin("clang++") } else { llvm_bin("clang") };
    let mut cmd = Command::new(&cc);
    cmd.arg("-O1");
    if cpp {
        cmd.arg("-std=c++17");
    }
    cmd.args(["-emit-llvm", "-S"]).arg(src).arg("-o").arg(out);
    let output = cmd.output().unwrap_or_else(|e| {
        panic!(
            "failed to spawn `{}`: {e}\n\
             (LLVM 19 required; install via `brew install llvm@19` and \
             ensure LLVM_SYS_191_PREFIX is set in .cargo/config.toml)",
            cc.display()
        )
    });
    assert!(
        output.status.success(),
        "clang failed for {}\nstderr:\n{}",
        src.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn awkvm_emit(ll: &Path, awk: &Path) {
    let output = Command::new(awkvm_bin())
        .arg(ll)
        .arg("-o")
        .arg(awk)
        .stderr(Stdio::null())
        .output()
        .expect("spawn awkvm");
    assert!(
        output.status.success(),
        "awkvm failed on {}\nstderr:\n{}",
        ll.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_gawk(awk: &Path, args: &[&str]) -> Outcome {
    let mut cmd = Command::new("gawk");
    cmd.env("LC_ALL", "C");
    cmd.arg("-f").arg(awk);
    if !args.is_empty() {
        cmd.arg("--");
        for a in args {
            cmd.arg(a);
        }
    }
    cmd.stdin(Stdio::null());
    let output = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn gawk (is it installed?): {e}"));
    Outcome {
        exit: output.status.code().unwrap_or(-1),
        stdout: output.stdout,
    }
}

fn check(stem: &str, ext: &str, args: &[&str], expect_exit: i32, expect_stdout: &[u8]) {
    let out = run_fixture(stem, ext, args);
    assert_eq!(
        out.exit, expect_exit,
        "[{stem}] exit code: got {}, expected {}",
        out.exit, expect_exit
    );
    assert_eq!(
        out.stdout.as_slice(),
        expect_stdout,
        "[{stem}] stdout mismatch\n--- got ---\n{}\n--- expected ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(expect_stdout),
    );
}

fn check_exit(stem: &str, ext: &str, args: &[&str], expect_exit: i32) {
    let out = run_fixture(stem, ext, args);
    assert_eq!(
        out.exit, expect_exit,
        "[{stem}] exit code: got {}, expected {}",
        out.exit, expect_exit
    );
}

// --- C fixtures ---------------------------------------------------------

#[test]
fn add() {
    check("add", "c", &[], 5, b"");
}

#[test]
fn agg() {
    check("agg", "c", &[], 46, b"");
}

// argv prints argv[0] whose content differs between a native binary and
// gawk-as-host (gawk's ARGV[0] is the gawk binary path); only assert the
// exit code, which is sum-of-arg-strlens and is host-independent.
#[test]
fn argv() {
    check_exit("argv", "c", &["foo", "bar"], 6);
}

#[test]
fn bits() {
    check("bits", "c", &[], 83, b"");
}

#[test]
fn buf() {
    check("buf", "c", &[], 120, b"");
}

#[test]
fn floats() {
    check("floats", "c", &[], 14, b"");
}

#[test]
fn fnptr() {
    check("fnptr", "c", &[], 11, b"");
}

#[test]
fn hello() {
    check("hello", "c", &[], 0, b"hello, world! 42\ndone\n");
}

#[test]
fn point() {
    check("point", "c", &[], 39, b"");
}

#[test]
fn str_example() {
    check("str", "c", &[], 98, b"");
}

#[test]
fn sum() {
    check("sum", "c", &[], 15, b"");
}

#[test]
fn table() {
    check("table", "c", &[], 70, b"");
}

// --- C++ fixtures -------------------------------------------------------

#[test]
fn cppio() {
    check("cppio", "cpp", &[], 0, b"");
}

#[test]
fn stdany() {
    check("stdany", "cpp", &[], 42, b"");
}

#[test]
fn stdmin() {
    check("stdmin", "cpp", &[], 3, b"");
}

#[test]
fn stdstr() {
    check("stdstr", "cpp", &[], 12, b"");
}

#[test]
fn stdvec() {
    check("stdvec", "cpp", &[], 10, b"");
}

#[test]
fn stdvecstr() {
    check("stdvecstr", "cpp", &[], 10, b"");
}

#[test]
fn throw_class() {
    check("throw_class", "cpp", &[], 42, b"");
}

#[test]
fn throw_int() {
    check("throw_int", "cpp", &[], 42, b"");
}
