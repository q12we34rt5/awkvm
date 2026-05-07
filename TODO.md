# TODO

Active backlog. Items move between sections as they progress.

For long-term direction (including stuff that's "wishlist" rather
than "next up"), see [ROADMAP.md](ROADMAP.md). For what's
user-visible-broken right now, see [LIMITATIONS.md](LIMITATIONS.md).

## In progress

_(empty — last item just landed)_

## Next major: v0.2.0 — C ↔ awk FFI

A coherent release theme: make awkvm bidirectional. C code can drop
to raw awk where useful, and awk scripts can call into compiled C.
Each item below is independently small; together they form the
v0.2.0 story.

- **[ffi]** `AWK_EXPORT` — awk-callable C library mode (~2-3 days,
  conceived 2026-05-07). `__attribute__((annotate("awkvm_export")))`
  marks a C function as exported. awkvm:
  1. Reads the annotation table, names the exports.
  2. Type-checks each export's signature; bails at codegen time on
     non-primitive args / returns. Initial scope: `int` / `long` /
     `unsigned` / `double` / `bool` / `char` / `void`. `const char*`
     and struct support deferred to a follow-up wrapper layer.
  3. Adds a `--library` mode that skips the
     `BEGIN { exit fn_main() }` line.
  4. Emits the user's chosen function name (no `fn_` prefix) so
     awk callers can `print sum_squares(10)` directly.

  Caller pattern: `gawk -f lib.awk -f script.awk`, where `lib.awk`
  is awkvm's output and `script.awk` calls the exported names.
  Note: clang rejects `extern "awk" {}` (verified — "unknown
  linkage language"), so we go via `annotate` instead.

Inline awk, `--link helpers.awk`, and `awkvm_body` annotate landed
(commits below). Remaining v0.2.0 work: AWK_EXPORT — builds on the
same `@llvm.global.annotations` parsing that `awkvm_body` now uses.

## Todo (next, post-v0.2.0)

- **[iostream]** Extend cin further: `>> std::string`, `>> unsigned`
  variants. The numeric tier (int / long / double) is wired; string
  needs libc++-layout-aware writes (similar story to `cout << string`
  but on the read side).
- **[iostream]** `std::getline(cin, str)`. Maps directly to gawk's
  `getline x < "/dev/stdin"`; should be a thin wrapper over the
  existing line-buffer state.

## Todo (later)

- **[iostream]** `<iomanip>` (setw / setfill / setprecision /
  hex / oct / dec / fixed / scientific). Per-stream format state in
  awk + per-helper consultation.
- **[iostream]** `<sstream>` + `<fstream>` + `cout.rdbuf(...)` swap
  as one streambuf-indirection unit. Two-level dispatch table
  (`ostream → streambuf id → target`) replaces the current
  cout/cerr/clog identity check.
- **[iostream]** `std::endl` proper support — needs ctype facet
  modeling so the inlined `widen('\n')` virtual dispatch resolves.
  Or stays as a documented permanent workaround (use `"\n"`).
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

## Probably won't do

- **[numerics]** i128 bignum — needs a full bignum runtime; cost
  outweighs use case for now.
- **[stdlib]** wide chars / `wstring` / locale facets — orthogonal
  rabbit hole, no clear win.

## Done (recent commits)

- _(this commit)_ `awkvm_body` annotation — `__attribute__((annotate(
  "awkvm_body(args):body")))` skips IR translation and emits the
  body verbatim. Annotation infra in `src/codegen/annotate.rs`
  reads `@llvm.global.annotations`; works on both full-body and
  declare-only functions. `docs/awkvm-body.md` for the cookbook.
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
