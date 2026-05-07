use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use tempfile::NamedTempFile;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_test(name: &str) {
    let dir = manifest_dir();

    let mut runtime_file = NamedTempFile::new().expect("tempfile");
    runtime_file
        .write_all(awkvm::runtime::RUNTIME.as_bytes())
        .expect("write runtime");
    runtime_file.flush().expect("flush runtime");

    let helper = dir.join("tests/runtime/test_helper.awk");
    let test = dir.join(format!("tests/runtime/{name}_test.awk"));

    let output = Command::new("gawk")
        .env("LC_ALL", "C")
        .arg("-f")
        .arg(runtime_file.path())
        .arg("-f")
        .arg(&helper)
        .arg("-f")
        .arg(&test)
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn gawk (is it installed? `brew install gawk`): {e}"));

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "runtime test '{name}' failed (exit={:?})\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}",
            output.status.code()
        );
    }
}

#[test] fn bitwise() { run_test("bitwise"); }
#[test] fn mem()     { run_test("mem"); }
#[test] fn str()     { run_test("str"); }
#[test] fn fp()      { run_test("fp"); }
#[test] fn cxa()     { run_test("cxa"); }
