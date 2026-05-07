# TODO

Active backlog. Items move between sections as they progress.

For long-term direction (including stuff that's "wishlist" rather
than "next up"), see [ROADMAP.md](ROADMAP.md). For what's
user-visible-broken right now, see [LIMITATIONS.md](LIMITATIONS.md).

## In progress

_(empty — picking up v0.3.0 next)_

## Next major: v0.3.0 — I/O subsystem

Coherent release theme: design awkvm's stream subsystem once, layer
both the libc and iostream APIs on top. The two API surfaces share
the same gawk primitives (`print > "file"`, `getline x < "file"`,
`cmd | getline`, `print | cmd`, `system()`); doing them together
means designing the stream model once instead of refactoring when
the second consumer comes online.

### Stream subsystem (foundation)

- fd-keyed table on the awk side mapping descriptor →
  (kind, gawk-target). Kinds: file, pipe-read, pipe-write, stdin,
  stdout, stderr, in-memory (for sstream).
- `/dev/stdin` line buffer reuse — the existing `cin` token reader
  is the closest precedent; lift it into a stream-keyed buffer so
  `getline(cin, ...)` and `fread(stdin, ...)` share state.
- Read / write / close primitives that both API surfaces call.

### libc bridge

- **[libc-io]** `system()` — gawk `system(s)` is already blocking
  fork+exec; one-line in `emit_libc`.
- **[libc-io]** `fopen` / `fwrite` / `fclose` write path — gawk
  `print > "file"` + `close("file")` already provides the
  underlying capability. Per-byte `printf "%c"` for binary data.
- **[libc-io]** `fread` line-buffered, slice into caller buffer.
  Uglier than fwrite — gawk's `getline` is line-based.
- **[libc-io]** `popen` / `pclose` — `cmd | getline` (read mode)
  / `print | cmd` (write mode). Standard POSIX C API; user code
  doesn't know it's awk.
- **[libc-io]** `scanf` / `sscanf` / `fscanf` — small parser on
  top of the format spec; reuses `_atoi` / `_atof`. ~one day.

### iostream bridge

- **[iostream]** `std::endl` proper support — needs ctype facet
  modeling so the inlined `widen('\n')` virtual dispatch resolves.
  Fallback: documented permanent workaround (use `"\n"`).
- **[iostream]** `std::getline(cin, str)`. Maps directly to gawk's
  `getline x < "/dev/stdin"`; thin wrapper over the line buffer.
- **[iostream]** Extend cin further: `>> std::string`,
  `>> unsigned` variants. Numeric tier (int / long / double) is
  wired; string needs libc++-layout-aware writes (similar story
  to `cout << string` but on the read side).
- **[iostream]** `<iomanip>` (setw / setfill / setprecision /
  hex / oct / dec / fixed / scientific). Per-stream format state
  on the awk side + per-helper consultation.
- **[iostream]** `<sstream>` + `<fstream>` + `cout.rdbuf(...)`
  swap as one streambuf-indirection unit. Two-level dispatch
  table (`ostream → streambuf id → target`) replaces the current
  cout/cerr/clog identity check.

### Demo target

End-to-end fixture exercising both surfaces in one program:
`fopen("file", "w"); fprintf(fp, ...)` and
`std::ofstream f; f << ...` writing to a verified output file.
Plus a roundtrip: write via libc, read via iostream (or vice
versa) to prove the descriptor table is shared.

Estimated effort: ~one week. Bigger than v0.2.0 (~3 days), but
release story is clean — "awkvm now has a real I/O subsystem".

## Todo (later)

- **[codegen]** i32 mul wraparound. clang -O1 can closed-form a loop
  into a polynomial like `r14 = r13 * 1431655766` (reciprocal of 3,
  scaled to 2^32) where the i32 result is the low 32 bits of the
  product mod 2^32. awkvm currently emits the multiplication as a
  raw `r13 * 1431655766` in awk, no truncation, so the result
  overflows the i32 lane and downstream arithmetic explodes
  (caught while building `awkvm_export.c` — `sum_squares(5)` came
  out as 17,179,869,239 instead of 55). Fix: wrap `mul` (and other
  width-bound arithmetic) through `_trunc(..., bits)` based on the
  IR result type. Affects any heavily-arithmetic IR that clang
  optimizes via the magic-number reciprocal trick.
- **[adt]** D2-D6 — lift `std::string` / `std::vector` / `std::map` to
  awk-native representation. Performance project.
- **[codegen]** Phi cycle parallel copy.
- **[codegen]** 32-bit ABI: parametrize pointer width on
  `module.target_triple` instead of the current 64-bit hardcode.
- **[exceptions]** `__cxa_end_catch` real dtor sequence — store the
  dtor pointer from `__cxa_throw` and call it on catch exit.
- **[exceptions]** `__vmi_class_type_info` (multi/virtual inheritance).
- **[docs]** README "Limitations" section, mining from
  `LIMITATIONS.md` once stable.

## Future release candidates (post-v0.3.0)

Themes ordered roughly by impact-to-effort, each its own minor
release:

- **v0.4.0 — Tooling / DX.** `awkvmcc` driver wrapping
  `clang -emit-llvm` + `llvm-link` + awkvm into a cc-style binary;
  source maps from awk → C/C++ line via IR `!dbg`; `awkvm trace`
  for BB-transition logging; better error messages with function /
  block context.
- **v0.5.0 — CI / release infrastructure.** GitHub Actions over
  (macOS / Linux × clang / gawk versions); Linux verification
  pass on the existing fixture suite; pre-built binary releases.
- **v0.6.0 — ADT lifting.** D2-D6 from "Todo (later)" — start
  with `std::string`, then `std::vector`, then `std::map`. Big
  performance lift for string-heavy programs.

## Probably won't do

- **[numerics]** i128 bignum — needs a full bignum runtime; cost
  outweighs use case for now.
- **[stdlib]** wide chars / `wstring` / locale facets — orthogonal
  rabbit hole, no clear win.

## Done (recent commits)

- `3066644` v0.2.0 release: CHANGELOG, version bump, tag
- `b2b65a1` Multi-library limitation documented — combine `.bc`
  files via `llvm-link` before `awkvm`, not awk outputs
  downstream via `gawk -f -f`
- `a0a6100` `awkvm_export` + `--library`. Annotation
  `__attribute__((annotate("awkvm_export")))` exposes a C function
  to outside awk via a bare-name wrapper that forwards into the
  existing `fn_<name>` body. `--library` flag skips the
  `BEGIN { exit fn_main() }` boot line. Type checker rejects
  non-primitive params / returns. `examples/awkvm_export.c` +
  `examples/awkvm_export_caller.awk` for the full round-trip;
  `docs/awkvm-export.md` for the cookbook.
- `cf23174` Renamed `awkvm_body` → `awkvm_fn` end-to-end
  (annotation key, fixture, doc, test) so the macro / file / key
  all line up.
- `308ed8a` `awkvm_fn` switched from `(args):body` to
  `(args) { body }` form + `AWKVM_FN(decl, body)` two-arg macro
  that auto-appends `;` so the user-side declaration looks like a
  natural awk function definition transposed onto C source.
- `0db3056` `awkvm_fn` annotation — `__attribute__((annotate(
  "awkvm_fn(args) { body }")))` skips IR translation and emits the
  body verbatim. Annotation infra in `src/codegen/annotate.rs`
  reads `@llvm.global.annotations`; works on both full-body and
  declare-only functions. `docs/awkvm-fn.md` for the cookbook.
- `a286ea5` link_basic_cpp fixture pinning the C++ extern "C" pattern
- `6961814` link-awk doc: extern "C" requirement
- `71c78d6` `awkvm --link helpers.awk` — concat a hand-written
  awk file into the emitted script; `function fn_<name>(...)` entries
  become callable from C-side `extern` declarations. `docs/link-awk.md`
  documents the convention.
- `16e61e0` `docs/inline-awk.md` cookbook + Features pointer in README
- `23b03fa` Inline awk asm escape unescaping (`\\` for backslash,
  `\HH` for hex, `$$` for literal `$`); `_str_to_mem` runtime helper
  closes the awk-string → C-string marshal direction; three new
  fixtures cover string round-trip, subprocess capture, and gawk regex
- `0103432` Inline awk via `__asm__("AWKVM:...")`: parser
  recovers asm/constraints from `.ll` text (LLVM C API hides them),
  codegen substitutes `%N` placeholders and emits raw awk
- `c2ce31e` README refresh: quick example, toolchain warning,
  doc links to ROADMAP / LIMITATIONS / CHANGELOG
- `84428d8` v0.1.0 release: README + CHANGELOG + stats_cli demo +
  `v0.1.0` git tag
- `2649a00` cin source via probe + `_ISTREAM_SRC` table;
  STREAM_GLOBALS metadata now uses `dest=` / `src=` prefix so
  ostream and istream globals share one machinery
- `39c2811` `cin >> long` / `cin >> double` extensions on top
  of the cin token reader
- `42cc55b` `cin >> int`: line buffer + token reader on the awk
  side; tests/examples gained an stdin-injection helper
- `d1c511c` cerr / clog dispatcher driven by probe-discovered
  globals (`STREAM_GLOBALS` table); iostream.awk no longer hardcodes
  libc++'s mangled names
- `e17a76a` codegen warns at compile time when user .ll has a
  probe-targeted libc++ symbol whose ABI tag doesn't match PROBE_MAP
- `72e3384` ostream long / unsigned int / unsigned long / bool /
  void\* overloads + cout_overloads fixture; `Constant::IntToPtr` /
  `PtrToInt` / `BitCast` constexprs now pass through `constant_str`
- `48ba99d` cout_char regression fixture (char-via-cstr probe)
- `c1a8068` Track ROADMAP / LIMITATIONS / TODO under git
- `abc8936` Route cerr / clog to /dev/stderr; tests assert on stderr
- `93daa6f` cout << double; cout_mixed fixture
- `36968b2` cout << const char* binding (cppio prints for real)
- `88f9b85` D1 probe pipeline + first iostream binding (cout << int)
- `ed9a15f` Split codegen.rs into 7 per-concern modules
- `decc1c3` LayoutCx — memoize named-struct size/align
- `122cc0b` libc helpers → runtime/libc.awk; user-shadow filtering
- `4b8e211` Awk-side unit tests for runtime helpers
- `20d708c` End-to-end regression test for examples/
- `b957c8a` /tests/ unignored, scratch moved to /local/
- `94c8b79` Runtime split into per-file `*.awk`
