# Changelog

## [0.3.0] — 2026-05-08

Theme: **I/O subsystem**. Single address-keyed stream model
(`_STREAM_*` tables) shared by libc `FILE*` and C++ `<fstream>`.
A program that mixes `fwrite(fp, ...)` and `ofstream f; f << ...`
reads back identically through either side.

### Added — stream foundation

- **`src/runtime/stream.awk`** — six `_STREAM_*` tables
  (`DEST` / `SRC` / `KIND` / `BUF` / `POS` / `EOF`) keyed by stream
  address. Five primitives that both API surfaces share:
  `_stream_open_w / open_r / close / read_byte / read_line`,
  `_stream_write_byte / write_str`. `KIND` picks the gawk redirect
  operator (`>` / `>>` / `<` / `cmd | getline` / `print | cmd`).

### Added — libc bridge

- **`fopen` / `fclose`** — `fopen` allocates a 1-byte FILE\* address,
  registers in stream tables; modes `"r"` / `"w"` / `"a"` (with
  optional `"b"` suffix). `"+"` forms not supported.
- **`fwrite` / `fread`** — bytewise loops over `MEM[]`. `fread`
  returns floor(bytes_read / size) on EOF mid-element.
- **`fputc` / `fputs` / `fgetc` / `fgets`** — single-byte and
  C-string convenience wrappers.
- **`popen` / `pclose`** — pipe streams; `pclose` returns gawk's
  `close()` status so child exit code surfaces.
- **`system`** — direct wrap of gawk's blocking `system()`.
- **`fprintf`** — same `_format` engine as `printf`, routed
  through `_stream_write_str(stream, ...)` instead of bare gawk
  `printf`.
- **`scanf` / `fscanf` / `sscanf`** — format-driven token reader
  on top of `_istream_read_token` / `_stream_read_byte`. `scanf`
  uses a lazily-registered `/dev/stdin` sentinel stream;
  `sscanf` preloads `_STREAM_BUF` from a cstring in MEM.

### Added — C++ `<fstream>`

- **`std::ofstream(path)` / `std::ifstream(path)`** — constructor
  probes register the path against the fstream's `this` AND
  `this + 8` (the rdbuf at +8 in libc++ layout) so both `<<` /
  `>>` (which key on `this`) and `f.close()` (which dispatches
  through the rdbuf) reach the same `_STREAM_*` entry.
- **`f << x` / `f >> x`** — inherited via base class; reuses the
  existing cout / cin probe-bindings for primitives. `cin >>` extended with `unsigned` / `unsigned long` overloads.
- **`f.write(buf, n)` / `f.put(c)`** — block + single-byte write.
  `write` shares the byte-loop helper with `<< "literal"`.
- **`f.read(buf, n)` / `f.gcount()` / `f.get()`** — block +
  single-byte read. `gcount()` isn't probed (clang -O1 inlines it
  to a load from offset +8); `_istream_read` / `_istream_get`
  `_store` the count at `MEM[stream+8]` so the inlined load picks
  it up.
- **`f.close()`** — routed through `basic_filebuf::close` (the
  mangled name `f.close()` actually resolves to). Closes the
  gawk-side handle; without this probe gawk's redirected output
  stays buffered and read-after-write to the same file in one
  program reads stale state.

### Added — robustness

- **Locale hard-fail at startup.** `BEGIN` block in
  `src/runtime/prelude.awk` checks `length("中") == 3` (true under
  `LC_ALL=C`, false under UTF-8 locales) and exits 2 with a
  pointer to the fix. awkvm's runtime treats strings as byte
  sequences; UTF-8 locales silently break byte-level I/O —
  failing loud is better than silent corruption.
- **Darwin `\x01_` asm-rename canonicalization.** Apple's
  `<stdio.h>` declares `fopen` / `fwrite` / `fputs` / `freopen`
  / `popen` with `__asm("_<name>")` aliases (LFS-compat
  artifact). clang emits these as `\x01_fopen` etc. in IR;
  `func_to_var` and the helper-name lookups now strip the
  `\x01_` prefix so `fn_fopen` / `fn_fwrite` / etc. resolve
  platform-agnostically. New `canonical_fn_name` helper in
  `src/codegen/names.rs`.

### Examples — reorganized into seven subdirectories

```
examples/
├── basics/      Phase 1-9 IR features
├── exceptions/  C++ EH
├── stdlib/      C++ stdlib smoke
├── iostream/    cout / cin probe bindings
├── cli/         CLI demos (stats_cli)
├── ffi/         v0.2.0 FFI features
└── io/          v0.3.0 I/O subsystem
```

Test stems updated to the `category/name` form (`basics/add`,
`io/io_mixed`, etc.); the fixture runner strips the subdir prefix
when naming temp `.ll` / `.awk` artifacts so they stay flat.

### Added — tests

`cargo test` runs **46 end-to-end fixtures** (was 37) plus 5
runtime unit-test bundles. New v0.3.0 fixtures:

- `io/file_io` — fopen / fwrite / fclose / fread round-trip
- `io/io_mixed` — libc and ofstream both writing, both readable
- `io/ifstream_extract` — ifstream + `>>` for primitives
- `io/block_io` — write / put / read / gcount / get(EOF)
- `io/fprintf_basic` — fprintf with `%d / %s / %.3f` round-trip
- `io/scanf_basic` — scanf reading three primitives from stdin
- `io/fscanf_basic` — fprintf write + fscanf read same FILE\*
- `io/sscanf_basic` — sscanf parsing primitives from a cstring
- `iostream/cin_unsigned` — `cin >> unsigned / unsigned long`

### Documentation

- New [`docs/io.md`](docs/io.md) — stream subsystem model, libc
  + C++ surface tables, locale requirement, `rdbuf`-swap
  limitation explanation, full v0.4.0 deferral list.
- README "Feature guides" section gains the io.md pointer.

### Known limitations / deferred to v0.4.0

iostream completion lands as a v0.4.0 theme:

- **`std::endl` proper.** clang inlines `endl` into ~6 calls
  including a virtual `widen('\n')` through ctype facet's vtable.
  Resolving that requires modeling libc++ locale machinery (or
  pattern-matching the inlined sequence — both v0.4.0 scope).
  Workaround: write `"\n"` literal.
- **`std::getline(is, str)` / `cin >> std::string`.** libc++
  `std::string` is SSO-laid-out in `MEM[]`; writing requires
  layout-aware code. Deferred.
- **`<iomanip>`** (`setw` / `setfill` / `setprecision` / `hex` /
  `oct` / `dec` / `fixed` / `scientific`). Per-stream format
  state + manipulator dispatch.
- **`<sstream>`** (in-memory streams) and **`cout.rdbuf(...)`
  swap** (two-level `ostream → streambuf id → target` dispatch
  table). Same machinery; ship together.
- **State queries** — `is.eof()` / `is.fail()` / `is.good()` /
  `is.bad()` fall through to libc++ linkonce_odr bodies that
  no-op in awkvm.

### Compatibility

No breaking changes to v0.2.0 codegen behavior. New `--library`
flag from v0.2.0 unchanged. Stream tables internal to runtime;
user IR doesn't see them.

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
