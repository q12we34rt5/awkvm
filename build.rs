// D1 probe pipeline.
//
// For every probes/src/*.cpp file, compile it with the user's clang++ to
// LLVM IR, then extract one (mangled_name, awk_template) pair per
// `awkvm_probe_<id>` function found in the resulting .ll. The pairs are
// emitted as `$OUT_DIR/probe_map.rs` and consumed by codegen via
// include!() so that calls to those mangled names get rewritten to the
// matching awk template instead of the default `fn_<sanitized>(...)`.
//
// Why this exists: hardcoded mangled names are brittle (they shift across
// libc++ / libstdc++ versions and 32 vs 64-bit ABI). Probes are compiled
// with the same toolchain that's compiling the user program, so the names
// captured here are guaranteed to match the names in user .ll output.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let probes_src = manifest_dir.join("probes/src");
    let templates_path = manifest_dir.join("probes/templates.txt");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));

    println!("cargo:rerun-if-changed=probes/src");
    println!("cargo:rerun-if-changed=probes/templates.txt");
    println!("cargo:rerun-if-changed=build.rs");

    let (call_templates, global_meta) = parse_templates(&templates_path);

    // Discover all probe sources. Any .cpp under probes/src/ is fair game.
    let mut sources: Vec<PathBuf> = Vec::new();
    if probes_src.is_dir() {
        for entry in fs::read_dir(&probes_src).expect("read probes/src") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("cpp") {
                sources.push(path);
            }
        }
    }
    sources.sort(); // stable output ordering

    let clangxx = clangxx_path();

    let mut call_entries: Vec<(String, String)> = Vec::new(); // (mangled, awk template)
    let mut global_entries: Vec<(String, String)> = Vec::new(); // (mangled, metadata)
    for src in &sources {
        let stem = src.file_stem().unwrap().to_string_lossy().into_owned();
        let ll = out_dir.join(format!("{stem}.ll"));
        compile_probe(&clangxx, src, &ll);
        for (probe_id, mangled) in extract_probe_calls(&ll) {
            if let Some(template) = call_templates.get(&probe_id) {
                call_entries.push((mangled, template.clone()));
            } else if let Some(meta) = global_meta.get(&probe_id) {
                global_entries.push((mangled, meta.clone()));
            } else {
                panic!(
                    "probe `awkvm_probe_{probe_id}` (in {}) has no entry in {}",
                    src.display(),
                    templates_path.display()
                );
            }
        }
    }

    // Stable order so the generated source is deterministic.
    call_entries.sort_by(|a, b| a.0.cmp(&b.0));
    global_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let map_path = out_dir.join("probe_map.rs");
    fs::write(&map_path, render_map(&call_entries, &global_entries))
        .expect("write probe_map.rs");
}

// Resolve clang++ via LLVM_SYS_191_PREFIX if it points at a real install,
// otherwise fall back to PATH lookup. Mirrors src/parser.rs's llvm_as_path
// so the project has one consistent way of locating LLVM tools — set the
// env var in .cargo/config.toml (or the shell) and everything follows.
fn clangxx_path() -> PathBuf {
    if let Ok(prefix) = std::env::var("LLVM_SYS_191_PREFIX") {
        let p = Path::new(&prefix).join("bin").join("clang++");
        if p.is_file() {
            return p;
        }
    }
    PathBuf::from("clang++")
}

fn compile_probe(clangxx: &Path, src: &Path, out_ll: &Path) {
    let status = Command::new(clangxx)
        .args(["-O1", "-std=c++17", "-emit-llvm", "-S"])
        .arg(src)
        .arg("-o")
        .arg(out_ll)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn `{}`: {e}", clangxx.display()));
    if !status.success() {
        panic!(
            "clang++ failed to compile probe {}: exit {}",
            src.display(),
            status.code().unwrap_or(-1)
        );
    }
}

// Parse templates.txt into (call_templates, global_meta). The two sigils:
//   probe_id => awk template      (call-site rewrite)
//   probe_id := metadata-string   (global-symbol annotation)
fn parse_templates(path: &Path) -> (HashMap<String, String>, HashMap<String, String>) {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut call = HashMap::new();
    let mut global = HashMap::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Order matters: `:=` would also match the `=` inside `=>`, so
        // try the longer/distinct form first.
        if let Some((id, body)) = line.split_once(":=") {
            global.insert(id.trim().to_string(), body.trim().to_string());
        } else if let Some((id, body)) = line.split_once("=>") {
            call.insert(id.trim().to_string(), body.trim().to_string());
        } else {
            panic!(
                "{}:{}: expected `<probe_id> => <template>` or \
                 `<probe_id> := <metadata>`, got: {raw}",
                path.display(),
                lineno + 1
            );
        }
    }
    (call, global)
}

// Walks an .ll file. For each `define ... @awkvm_probe_<id>(...)`, finds
// the first `@_Z<sym>(` call inside its body (up to the matching `}` line)
// and yields (id, mangled).
fn extract_probe_calls(ll: &Path) -> Vec<(String, String)> {
    let text = fs::read_to_string(ll)
        .unwrap_or_else(|e| panic!("read {}: {e}", ll.display()));
    let mut out = Vec::new();
    let mut in_probe: Option<String> = None;
    let mut found_for_current = false;

    for line in text.lines() {
        if let Some(name) = parse_probe_define(line) {
            in_probe = Some(name);
            found_for_current = false;
            continue;
        }
        if in_probe.is_some() {
            // End of function body — bare `}` at column 0.
            if line == "}" {
                let probe_id = in_probe.take().unwrap();
                if !found_for_current {
                    panic!(
                        "probe `awkvm_probe_{probe_id}` body has no @_Z* call \
                         in {} — clang may have inlined it; consider adding \
                         __attribute__((noinline)) or check for typos",
                        ll.display()
                    );
                }
                continue;
            }
            if !found_for_current
                && let Some(mangled) = first_mangled_call(line)
            {
                let probe_id = in_probe.clone().unwrap();
                out.push((probe_id, mangled));
                found_for_current = true;
            }
        }
    }
    out
}

// Recognize lines like:
//   define void @awkvm_probe_ostream_int(...) ... {
fn parse_probe_define(line: &str) -> Option<String> {
    let rest = line.strip_prefix("define ")?;
    let at = rest.find("@awkvm_probe_")?;
    let after = &rest[at + "@awkvm_probe_".len()..];
    let end = after.find('(')?;
    Some(after[..end].to_string())
}

// Find the first `@_Z<word>(` reference on a line that's a call/invoke
// site inside a probe body. We accept any line with `@_Z` because probe
// bodies are simple — the only thing referencing `@_Z*` is the call we
// care about.
fn first_mangled_call(line: &str) -> Option<String> {
    let at = line.find("@_Z")?;
    let after = &line[at + 1..]; // skip '@'
    let end = after
        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
        .unwrap_or(after.len());
    Some(after[..end].to_string())
}

fn render_map(
    call_entries: &[(String, String)],
    global_entries: &[(String, String)],
) -> String {
    let mut s = String::new();
    s.push_str("// Generated by build.rs from probes/. Do not edit by hand.\n");

    s.push_str("\npub(super) static PROBE_MAP: &[(&str, &str)] = &[\n");
    for (mangled, template) in call_entries {
        s.push_str(&format!("    ({:?}, {:?}),\n", mangled, template));
    }
    s.push_str("];\n");

    // STREAM_GLOBALS: mangled global name -> ostream output target
    // (consumed by emit_globals_init when the user's IR references the
    // matching extern global).
    s.push_str("\npub(super) static STREAM_GLOBALS: &[(&str, &str)] = &[\n");
    for (mangled, meta) in global_entries {
        s.push_str(&format!("    ({:?}, {:?}),\n", mangled, meta));
    }
    s.push_str("];\n");

    s
}
