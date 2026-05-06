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

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn awkvm_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_awkvm"))
}

// Mirrors src/parser.rs::llvm_as_path: try LLVM_SYS_191_PREFIX/bin/<name>
// first (the project's pinned toolchain, set in .cargo/config.toml), then
// fall back to a bare name so PATH lookup picks up whatever clang the host
// has installed.
fn llvm_bin(name: &str) -> PathBuf {
    if let Some(prefix) = option_env!("LLVM_SYS_191_PREFIX") {
        let p = PathBuf::from(prefix).join("bin").join(name);
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(name)
}

struct Outcome {
    exit: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_fixture(stem: &str, ext: &str, args: &[&str]) -> Outcome {
    run_fixture_with_stdin(stem, ext, args, b"")
}

fn run_fixture_with_stdin(stem: &str, ext: &str, args: &[&str], stdin: &[u8]) -> Outcome {
    let src = manifest_dir().join("examples").join(format!("{stem}.{ext}"));
    assert!(src.exists(), "fixture missing: {}", src.display());

    let tmp = TempDir::new().expect("tempdir");
    let ll = tmp.path().join(format!("{stem}.ll"));
    let awk = tmp.path().join(format!("{stem}.awk"));

    compile_to_ll(&src, ext == "cpp", &ll);
    awkvm_emit(&ll, &awk);
    run_gawk(&awk, args, stdin)
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

fn run_gawk(awk: &Path, args: &[&str], stdin: &[u8]) -> Outcome {
    let mut cmd = Command::new("gawk");
    cmd.env("LC_ALL", "C");
    cmd.arg("-f").arg(awk);
    if !args.is_empty() {
        cmd.arg("--");
        for a in args {
            cmd.arg(a);
        }
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn gawk (is it installed?): {e}"));
    child
        .stdin
        .as_mut()
        .expect("gawk stdin")
        .write_all(stdin)
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait gawk");
    Outcome {
        exit: output.status.code().unwrap_or(-1),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

// Default check: assert exit + stdout, and demand stderr is empty.
// Anything intentionally writing to stderr (e.g. cerr fixtures) uses
// check_streams below; asserting empty by default catches stray gawk
// warnings or accidental cerr/clog leaks.
fn check(stem: &str, ext: &str, args: &[&str], expect_exit: i32, expect_stdout: &[u8]) {
    check_streams(stem, ext, args, expect_exit, expect_stdout, b"");
}

fn check_streams(
    stem: &str,
    ext: &str,
    args: &[&str],
    expect_exit: i32,
    expect_stdout: &[u8],
    expect_stderr: &[u8],
) {
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
    assert_eq!(
        out.stderr.as_slice(),
        expect_stderr,
        "[{stem}] stderr mismatch\n--- got ---\n{}\n--- expected ---\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(expect_stderr),
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

fn check_with_stdin(
    stem: &str,
    ext: &str,
    args: &[&str],
    stdin: &[u8],
    expect_exit: i32,
    expect_stdout: &[u8],
) {
    let out = run_fixture_with_stdin(stem, ext, args, stdin);
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
    check("cppio", "cpp", &[], 0, b"hello, awkvm\n");
}

#[test]
fn cout_int() {
    check("cout_int", "cpp", &[], 0, b"42");
}

#[test]
fn cout_char() {
    check("cout_char", "cpp", &[], 0, b"A");
}

#[test]
fn cout_mixed() {
    check("cout_mixed", "cpp", &[], 0, b"x = 5, pi = 3.14\n");
}

#[test]
fn cout_overloads() {
    check(
        "cout_overloads",
        "cpp",
        &[],
        0,
        b"1234567890 4000000000 12345 1 0x42\n",
    );
}

#[test]
fn cout_cerr() {
    check_streams(
        "cout_cerr",
        "cpp",
        &[],
        0,
        b"out: 1\n",
        b"err: 2\n",
    );
}

#[test]
fn cin_int() {
    check_with_stdin("cin_int", "cpp", &[], b"3 5\n", 0, b"8\n");
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
