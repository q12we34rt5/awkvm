# Changelog

## [0.2.0] — 2026-05-07

Theme: **C ↔ awk FFI**. Four pieces ship together to make awkvm
bidirectional — C code can drop into raw awk where useful, and
external awk scripts can call into compiled C.

### Added — FFI surface

- **Inline awk via `__asm__("AWKVM:...")`** ([docs/inline-awk.md](docs/inline-awk.md)).
  Statement-level awk inside an otherwise IR-translated function.
  Reaches gawk's full surface (regex, subprocess pipes, file I/O,
  coprocess, `getline`, time built-ins, ...) without per-feature
  stubs. Parser recovers asm + constraints from `.ll` text since
  the LLVM C API doesn't expose them; codegen substitutes `%N`
  operand placeholders. Escape unescaping handles `\\` for
  backslash, `\HH` for hex, and `$$` for literal `$`. Companion
  runtime helper `_str_to_mem` closes the awk-string → C-string
  marshal direction.
- **`awkvm --link helpers.awk`** ([docs/link-awk.md](docs/link-awk.md)).
  Concatenate one or more hand-written awk files into the emitted
  script. Functions defined as `function fn_<name>(...)` become
  callable from C-side `extern <T> <name>(...)` declarations and
  from inline awk via the same `fn_<name>`. The `--link` flag is
  repeatable. Declare-only stubs are suppressed for any name that
  the linked awk provides, so the user-supplied implementation
  isn't shadowed by an empty default.
- **`awkvm_fn` body annotation** ([docs/awkvm-fn.md](docs/awkvm-fn.md)).
  `__attribute__((annotate("awkvm_fn(args) { body }")))` lets a C
  function carry its awk implementation in the annotation; awkvm
  uses the annotation as the body and skips IR translation. The
  `(args)` rename list maps awk parameters to readable names
  (clang -O1 strips C-source param names from the IR). Pair with
  the `AWKVM_FN(decl, body)` two-arg macro for declarations that
  read like awk function definitions transposed onto C source.
  Body lines get dedented and blank-line-trimmed before emission.
  Annotation infrastructure (`src/codegen/annotate.rs`) walks
  `@llvm.global.annotations` and is shared with `awkvm_export`.
- **`awkvm_export` + `--library`** ([docs/awkvm-export.md](docs/awkvm-export.md)).
  The inverse direction: `__attribute__((annotate("awkvm_export")))`
  on a C function plus `awkvm --library` produces a gawk-loadable
  library that an external awk script can call into via the bare
  C name. Caller pattern: `gawk -f lib.awk -f script.awk`. v0.2.0
  ABI is primitive-only (int / long / unsigned / double / bool /
  char / void); pointer / struct support deferred to a follow-up
  marshaling layer. Multiple awkvm-generated libraries can't be
  combined downstream — use `llvm-link` to merge `.bc` files
  before invoking awkvm. Type checker bails at codegen time on
  non-primitive signatures with a clear error message.

### Added — tests

`cargo test` now runs 37 end-to-end fixtures (was 29) plus 5
runtime unit-test bundles. New FFI-focused fixtures:

- `inline_awk` — `%N` operand substitution and constraint parsing
- `inline_awk_str` — C-string → awk-string round-trip via `_cstr` /
  `_str_to_mem`
- `inline_awk_pipe` — subprocess capture (`cmd | getline`)
- `inline_awk_regex` — gawk regex (`gsub`) reachable from C
- `link_basic` — C-side `extern int clip(...)` resolved via linked
  awk
- `link_basic_cpp` — same with `extern "C"` wrapping
- `awkvm_fn` — annotation-driven body with multi-line awk source
- `awkvm_export` — bare-name wrappers exposed to a hand-written
  caller awk

### Documentation

`docs/` directory introduced; per-feature recipes for each FFI
surface area. README "Feature guides" section indexes them.
LIMITATIONS.md gains an FFI section pinning the multi-library
restriction and the primitive-only export ABI.

### Known issues filed for follow-up

- **i32 mul wraparound.** clang -O1 occasionally closed-forms a
  loop into a polynomial like `r14 = r13 * 1431655766` (reciprocal
  of 3, scaled to 2^32) where the i32 result is the low 32 bits
  of the product. awkvm currently doesn't truncate the multiply,
  so the intermediate overflows the i32 lane and downstream
  arithmetic explodes. Surfaced while building the
  `awkvm_export.c` fixture. Filed under `[codegen]` in TODO.

### Compatibility

No breaking changes to v0.1.0 IR translation behavior. New CLI
flags (`--link`, `--library`) are additive; default invocation
(`awkvm input.bc -o out.awk`) is unchanged.

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
