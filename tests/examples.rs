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
    run_fixture_full(stem, ext, args, stdin, &[])
}

fn run_fixture_full(
    stem: &str,
    ext: &str,
    args: &[&str],
    stdin: &[u8],
    link: &[&str],
) -> Outcome {
    let src = manifest_dir().join("examples").join(format!("{stem}.{ext}"));
    assert!(src.exists(), "fixture missing: {}", src.display());

    // Strip subdir prefix when naming temp artifacts — `examples/<cat>/<name>.<ext>`
    // is the source path; the `.ll` / `.awk` files only need a unique basename.
    let basename = stem.rsplit('/').next().unwrap_or(stem);
    let tmp = TempDir::new().expect("tempdir");
    let ll = tmp.path().join(format!("{basename}.ll"));
    let awk = tmp.path().join(format!("{basename}.awk"));

    compile_to_ll(&src, ext == "cpp", &ll);
    awkvm_emit(&ll, &awk, link);
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

fn awkvm_emit(ll: &Path, awk: &Path, link: &[&str]) {
    let mut cmd = Command::new(awkvm_bin());
    cmd.arg(ll).arg("-o").arg(awk);
    for name in link {
        cmd.arg("--link")
            .arg(manifest_dir().join("examples").join(name));
    }
    let output = cmd.stderr(Stdio::null()).output().expect("spawn awkvm");
    assert!(
        output.status.success(),
        "awkvm failed on {}\nstderr:\n{}",
        ll.display(),
        String::from_utf8_lossy(&output.stderr)
    );
}

// `awkvm --library`: compile .c/.cpp into a gawk-loadable library, then
// run a hand-written awk script that calls into it via the bare-name
// wrappers emitted for `awkvm_export`-annotated C functions.
fn run_export_fixture(stem: &str, ext: &str) -> Outcome {
    let src = manifest_dir().join("examples").join(format!("{stem}.{ext}"));
    let caller = manifest_dir()
        .join("examples")
        .join(format!("{stem}_caller.awk"));
    assert!(src.exists(), "fixture missing: {}", src.display());
    assert!(caller.exists(), "caller missing: {}", caller.display());

    let basename = stem.rsplit('/').next().unwrap_or(stem);
    let tmp = TempDir::new().expect("tempdir");
    let ll = tmp.path().join(format!("{basename}.ll"));
    let lib_awk = tmp.path().join(format!("{basename}.awk"));

    compile_to_ll(&src, ext == "cpp", &ll);

    let mut cmd = Command::new(awkvm_bin());
    cmd.arg(&ll).arg("-o").arg(&lib_awk).arg("--library");
    let output = cmd.stderr(Stdio::null()).output().expect("spawn awkvm");
    assert!(
        output.status.success(),
        "awkvm --library failed on {}\nstderr:\n{}",
        ll.display(),
        String::from_utf8_lossy(&output.stderr)
    );

    let mut cmd = Command::new("gawk");
    cmd.env("LC_ALL", "C");
    cmd.arg("-f").arg(&lib_awk).arg("-f").arg(&caller);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().expect("spawn gawk");
    child.stdin.as_mut().unwrap().write_all(b"").unwrap();
    let output = child.wait_with_output().expect("wait gawk");
    Outcome {
        exit: output.status.code().unwrap_or(-1),
        stdout: output.stdout,
        stderr: output.stderr,
    }
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
    check_full(stem, ext, args, stdin, expect_exit, expect_stdout, b"");
}

fn check_full(
    stem: &str,
    ext: &str,
    args: &[&str],
    stdin: &[u8],
    expect_exit: i32,
    expect_stdout: &[u8],
    expect_stderr: &[u8],
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
    assert_eq!(
        out.stderr.as_slice(),
        expect_stderr,
        "[{stem}] stderr mismatch\n--- got ---\n{}\n--- expected ---\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(expect_stderr),
    );
}

// --- C fixtures ---------------------------------------------------------

#[test]
fn add() {
    check("basics/add", "c", &[], 5, b"");
}

#[test]
fn agg() {
    check("basics/agg", "c", &[], 46, b"");
}

// argv prints argv[0] whose content differs between a native binary and
// gawk-as-host (gawk's ARGV[0] is the gawk binary path); only assert the
// exit code, which is sum-of-arg-strlens and is host-independent.
#[test]
fn argv() {
    check_exit("basics/argv", "c", &["foo", "bar"], 6);
}

#[test]
fn bits() {
    check("basics/bits", "c", &[], 83, b"");
}

#[test]
fn buf() {
    check("basics/buf", "c", &[], 120, b"");
}

#[test]
fn floats() {
    check("basics/floats", "c", &[], 14, b"");
}

#[test]
fn fnptr() {
    check("basics/fnptr", "c", &[], 11, b"");
}

#[test]
fn hello() {
    check("basics/hello", "c", &[], 0, b"hello, world! 42\ndone\n");
}

#[test]
fn point() {
    check("basics/point", "c", &[], 39, b"");
}

#[test]
fn str_example() {
    check("basics/str", "c", &[], 98, b"");
}

#[test]
fn sum() {
    check("basics/sum", "c", &[], 15, b"");
}

#[test]
fn table() {
    check("basics/table", "c", &[], 70, b"");
}

#[test]
fn inline_awk() {
    // 7 * 7 = 49; 3 * 4 + 5 = 17. Verifies `$N` substitution covers
    // both output (`%0`) and several input operands (`%1` / `%2` / `%3`).
    check("ffi/inline_awk", "c", &[], 0, b"sq=49 r=17\n");
}

#[test]
fn inline_awk_str() {
    // C string → awk string → toupper → back to C string. Exercises
    // _cstr (MEM→awk) and _str_to_mem (awk→MEM) marshaling helpers
    // through a single inline-awk site.
    check("ffi/inline_awk_str", "c", &[], 0, b"HELLO, WORLD\n");
}

#[test]
fn inline_awk_pipe() {
    // `cmd | getline line; close(cmd)` — subprocess stdout captured
    // into an awk variable, then handed back to C as a char*.
    // Uses `printf hello` (no trailing newline) for deterministic output.
    check(
        "ffi/inline_awk_pipe",
        "c",
        &[],
        0,
        b"subprocess said: hello\n",
    );
}

#[test]
fn inline_awk_regex() {
    // gawk regex (`gsub`) reachable through inline awk. C string in,
    // C string out via the same _cstr / _str_to_mem marshal pair.
    check("ffi/inline_awk_regex", "c", &[], 0, b"hell0 w0rld\n");
}

#[test]
fn ifstream_extract() {
    // ofstream writes three primitives space-separated; ifstream
    // reads them back via `>>`. Proves ifstream's extraction path
    // works through cin's `_istream_*` bindings via base-class
    // inheritance — what's NOT covered is `read(buf, n)` /
    // `gcount()` block-read (deferred to v0.4.0).
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let path = tmp.path().join("data.txt");
    let path_str = path.to_str().expect("path utf8");
    let out = run_fixture("io/ifstream_extract", "cpp", &[path_str]);
    let expected = b"a=42 b=3.14 c=1234567890\n";
    assert_eq!(out.exit, 0, "[ifstream_extract] exit: {}", out.exit);
    assert_eq!(
        out.stdout.as_slice(),
        expected,
        "[ifstream_extract] stdout mismatch\n--- got ---\n{}\n--- expected ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(expected),
    );
    assert!(
        out.stderr.is_empty(),
        "[ifstream_extract] unexpected stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn io_mixed() {
    // v0.3.0 demo: libc fopen/fwrite/fclose AND C++ ofstream/<< on
    // the same stream subsystem. Two paths to two files; readback
    // proves the underlying content is what we wrote (not stale or
    // buffered) — that's the integration test the unified
    // _STREAM_* tables were built for.
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let path_a = tmp.path().join("a.txt");
    let path_b = tmp.path().join("b.txt");
    let out = run_fixture(
        "io/io_mixed",
        "cpp",
        &[
            path_a.to_str().expect("path_a utf8"),
            path_b.to_str().expect("path_b utf8"),
        ],
    );
    let expected = b"a: from libc\nb: from ofstream\n";
    assert_eq!(out.exit, 0, "[io_mixed] exit: {}", out.exit);
    assert_eq!(
        out.stdout.as_slice(),
        expected,
        "[io_mixed] stdout mismatch\n--- got ---\n{}\n--- expected ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(expected),
    );
    assert!(
        out.stderr.is_empty(),
        "[io_mixed] unexpected stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn file_io() {
    // fopen → fwrite ×2 → fclose → fopen → fread → fclose → printf.
    // Path lives in a per-test TempDir so the file is auto-cleaned;
    // the read-side fread asks for 32 bytes vs the 12 actually
    // written, exercising the EOF return path.
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let path = tmp.path().join("io.txt");
    let path_str = path.to_str().expect("non-utf8 path");
    let out = run_fixture("io/file_io", "c", &[path_str]);
    let expected = b"read 12 bytes:\nhello\nworld\n";
    assert_eq!(out.exit, 0, "[file_io] exit: {}", out.exit);
    assert_eq!(
        out.stdout.as_slice(),
        expected,
        "[file_io] stdout mismatch\n--- got ---\n{}\n--- expected ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(expected),
    );
    assert!(
        out.stderr.is_empty(),
        "[file_io] unexpected stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn awkvm_export() {
    // `awkvm --library` + `__attribute__((annotate("awkvm_export")))`:
    // expose three primitive-only functions to an external awk caller
    // via bare-name wrappers. Caller runs entirely in gawk; the
    // wrappers forward into `fn_<name>` bodies emitted from C.
    let out = run_export_fixture("ffi/awkvm_export", "c");
    assert_eq!(out.exit, 0, "[awkvm_export] exit: {}", out.exit);
    let expected = b"triangle(5) = 15\n\
                     triangle(10) = 55\n\
                     clipd(2.7, 0, 1) = 1.0\n\
                     clipd(-5, 0, 10) = 0.0\n\
                     gcd(48, 18) = 6\n\
                     gcd(-12, 8) = 4\n";
    assert_eq!(
        out.stdout.as_slice(),
        expected,
        "[awkvm_export] stdout mismatch\n--- got ---\n{}\n--- expected ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(expected),
    );
    assert!(
        out.stderr.is_empty(),
        "[awkvm_export] unexpected stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn awkvm_fn() {
    // `__attribute__((annotate("awkvm_fn(args) { body }")))` — awkvm
    // emits the annotation's awk body in place of IR translation.
    // Inputs come from argv so clang can't const-fold the clip()
    // calls — that's the catch the previous version of this test
    // missed (it passed by accident even when fn_clip referenced
    // undefined awk variables, because the body was never reached).
    check(
        "ffi/awkvm_fn",
        "c",
        &["0", "10", "-3", "5", "20"],
        0,
        b"clip(-3) = 0\nclip(5) = 5\nclip(20) = 10\n",
    );
}

#[test]
fn link_basic() {
    // `--link link_basic.awk` provides a `fn_clip` definition; the C
    // side declares `extern int clip(...)` and calls it directly.
    let out = run_fixture_full("ffi/link_basic", "c", &[], b"", &["ffi/link_basic.awk"]);
    assert_eq!(out.exit, 0);
    assert_eq!(
        out.stdout.as_slice(),
        b"clip(  5,  0, 10) = 5\nclip( -3,  0, 10) = 0\nclip( 20,  0, 10) = 10\n"
    );
    assert_eq!(out.stderr.as_slice(), b"");
}

#[test]
fn link_basic_cpp() {
    // Same fixture compiled as C++. Pins the `extern "C"` requirement
    // documented in docs/link-awk.md — without that wrap clang++
    // mangles `clip` to `_Z4clipiii` and the linked `fn_clip` doesn't
    // match, so this test would print "0 0 0". Shares the same helper
    // .awk file as link_basic.
    let out = run_fixture_full("ffi/link_basic_cpp", "cpp", &[], b"", &["ffi/link_basic.awk"]);
    assert_eq!(out.exit, 0);
    assert_eq!(
        out.stdout.as_slice(),
        b"clip(  5,  0, 10) = 5\nclip( -3,  0, 10) = 0\nclip( 20,  0, 10) = 10\n"
    );
    assert_eq!(out.stderr.as_slice(), b"");
}

// --- C++ fixtures -------------------------------------------------------

#[test]
fn cppio() {
    check("iostream/cppio", "cpp", &[], 0, b"hello, awkvm\n");
}

#[test]
fn cout_int() {
    check("iostream/cout_int", "cpp", &[], 0, b"42");
}

#[test]
fn cout_char() {
    check("iostream/cout_char", "cpp", &[], 0, b"A");
}

#[test]
fn cout_mixed() {
    check("iostream/cout_mixed", "cpp", &[], 0, b"x = 5, pi = 3.14\n");
}

#[test]
fn cout_overloads() {
    check(
        "iostream/cout_overloads",
        "cpp",
        &[],
        0,
        b"1234567890 4000000000 12345 1 0x42\n",
    );
}

#[test]
fn cout_cerr() {
    check_streams(
        "iostream/cout_cerr",
        "cpp",
        &[],
        0,
        b"out: 1\n",
        b"err: 2\n",
    );
}

#[test]
fn cin_int() {
    check_with_stdin("iostream/cin_int", "cpp", &[], b"3 5\n", 0, b"8\n");
}

#[test]
fn cin_unsigned() {
    // unsigned (32-bit) tier: 4294967290 is just below 2^32 — fits
    // exactly in awk's double, round-trips through the signed-model
    // wrap. unsigned long (64-bit) reads in the same value via the
    // 64-bit helper. Output uses cout's unsigned overload.
    check_with_stdin(
        "iostream/cin_unsigned",
        "cpp",
        &[],
        b"4294967290 4294967290\n",
        0,
        b"4294967290 4294967290\n",
    );
}

#[test]
fn cin_mixed() {
    // 1234567890 fits comfortably in i64 (< 2^53), so awk's number
    // representation reads it back exactly. 3.14 round-trips with
    // %g formatting.
    check_with_stdin(
        "iostream/cin_mixed",
        "cpp",
        &[],
        b"1234567890 3.14\n",
        0,
        b"1234567890 3.14\n",
    );
}

#[test]
fn stats_cli() {
    // Demo fixture: read N then N ints, output stats. Exercises every
    // recognized iostream primitive (cin >> int, cout << with int /
    // long / double / cstr, cerr for error path).
    check_with_stdin(
        "cli/stats_cli",
        "cpp",
        &[],
        b"5\n10 -3 7 0 8\n",
        0,
        b"n=5 sum=22 min=-3 max=10 mean=4.4\n",
    );
}

#[test]
fn stats_cli_error() {
    // Negative N → cerr message, exit 1. Demonstrates the cerr
    // dispatch routes to /dev/stderr (verified by separate stderr
    // assertion below).
    check_full(
        "cli/stats_cli",
        "cpp",
        &[],
        b"-1\n",
        1,
        b"",
        b"error: n must be positive\n",
    );
}

#[test]
fn stdany() {
    check("stdlib/stdany", "cpp", &[], 42, b"");
}

#[test]
fn stdmin() {
    check("stdlib/stdmin", "cpp", &[], 3, b"");
}

#[test]
fn stdstr() {
    check("stdlib/stdstr", "cpp", &[], 12, b"");
}

#[test]
fn stdvec() {
    check("stdlib/stdvec", "cpp", &[], 10, b"");
}

#[test]
fn stdvecstr() {
    check("stdlib/stdvecstr", "cpp", &[], 10, b"");
}

#[test]
fn throw_class() {
    check("exceptions/throw_class", "cpp", &[], 42, b"");
}

#[test]
fn throw_int() {
    check("exceptions/throw_int", "cpp", &[], 42, b"");
}
