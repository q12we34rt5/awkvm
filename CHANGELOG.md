# Changelog

## [0.1.0] — 2026-05-07

First tagged release. Compiles a meaningful subset of C / C++ to gawk.

### What works

- **Core IR**: integer / float arithmetic, control flow (br / condbr /
  switch / phi), bitwise ops with awk's `and`/`or`/`xor`/`lshift`/`rshift`,
  width conversions, direct + indirect calls (id-keyed dispatcher),
  byte-addressed memory (alloca / load / store / GEP, type punning),
  globals with constant initializers, first-class aggregates
  (extractvalue / insertvalue).
- **Floats**: single + double precision with IEEE 754 load/store.
  Subnormals collapse to 0; NaN / Inf not preserved across mem.
- **C++ exceptions**: throw / catch via Itanium ABI, single-inheritance
  RTTI matching, minimal `__cxa_*` runtime. `__cxa_end_catch` is a no-op
  (dtors don't run); multi / virtual inheritance not handled.
- **libc bridge**: `printf` / `puts` / `putchar` / `malloc` / `free` /
  `exit` / `abort` / `mem*` / `strlen` / `strcmp` / `atoi` / `atof` /
  Itanium operator new / delete variants. `free` and `delete` are no-ops
  (bump allocator).
- **C++ stdlib smoke**: `std::min`, `std::vector<int>`, `std::string`,
  `std::vector<std::string>`, `std::any` round-trip through awkvm.
  Real ~500-line C++ project (Q3Engine `plane_cube_shadow`) renders
  byte-identical to native.
- **iostream via probe-based binding** (Phase 13, this release):
  `cout` / `cerr` / `clog` with `int` / `long` / `unsigned int` /
  `unsigned long` / `bool` / `void*` / `double` / `char` /
  `const char*`. `cin >>` for `int` / `long` / `double` with
  token-aware line buffering. `std::endl` / `ofstream` / `ifstream` /
  `sstream` / `iomanip` not yet wired — see LIMITATIONS.md.

### Architecture

- **Probe pipeline** (`probes/` + `build.rs`): build-time probes
  compile through the user's clang to discover Itanium-mangled
  libc++ symbol names. The captured table feeds codegen, which
  rewrites recognized stdlib calls into awk templates instead of
  routing them through declare-only stubs. The resulting binding
  follows the toolchain that builds awkvm — no hardcoded mangled
  names, no symbol shifts on libc++ minor versions to keep up with.
- **Toolchain mismatch detection**: if a user `.ll` references a
  probe-targeted symbol whose ABI tag doesn't match `PROBE_MAP`,
  codegen warns loudly at compile time instead of silently
  falling through to a no-op stub.
- **Codegen module split**: `src/codegen/` divided into per-concern
  files (`names.rs`, `types.rs`, `globals.rs`, `func.rs`, `mem.rs`,
  `call.rs`); `LayoutCx` memoizes named-struct size / align.
- **Runtime split**: `src/runtime/*.awk` files concatenated via
  `include_str!`; libc / Itanium-ABI helpers live in `runtime/libc.awk`
  with user-shadow filtering (a custom `operator new` in user code
  takes precedence over the default helper).

### Tests

- `cargo test` runs 29 end-to-end fixtures (compile .c/.cpp →
  awkvm → gawk, assert on exit code + stdout + stderr) plus 5
  awk-side unit-test bundles for the runtime helpers (77
  individual assertions covering bitwise / mem / str / fp / cxa).

### Documentation

- `README.md` for project intro and usage.
- `ROADMAP.md` for strategic direction (Phase narrative, future
  work catalogued by impact / effort).
- `LIMITATIONS.md` for user-visible behavior gaps.
- `TODO.md` for active development backlog.

### Known limitations

See [LIMITATIONS.md](LIMITATIONS.md) for the full list. Notable:

- `free` / `delete` are no-ops (bump allocator never reclaims).
- 64-bit ABI hardcoded; 32-bit not supported.
- Phi cycles resolved sequentially (swap pattern broken).
- `std::endl` falls through silently (use `"\n"` literal).
- `ofstream` / `ifstream` / `sstream` not wired.
- Toolchain coupling: user `.ll` must come from the same clang as
  awkvm's build (same libc++ version), or recognized stdlib calls
  silently degrade.

### Tested toolchain

clang 19 + libc++ 19 + macOS arm64. Other combinations are
best-effort.
