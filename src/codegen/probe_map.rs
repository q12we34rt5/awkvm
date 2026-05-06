// Stub that pulls in build.rs's generated probe_map.rs from OUT_DIR.
// The included file declares `pub(super) static PROBE_MAP: &[(&str, &str)]`,
// which other codegen submodules then access via `super::probe_map::PROBE_MAP`.
include!(concat!(env!("OUT_DIR"), "/probe_map.rs"));
