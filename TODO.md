# TODO

Active backlog. Items move between sections as they progress.

For long-term direction (including stuff that's "wishlist" rather
than "next up"), see [ROADMAP.md](ROADMAP.md). For what's
user-visible-broken right now, see [LIMITATIONS.md](LIMITATIONS.md).

## In progress

- **[cleanup]** Drop hardcoded `/opt/homebrew/opt/llvm@19` fallbacks
  in `build.rs` and `tests/examples.rs`. Stash at `stash@{0}`. Use
  `parser.rs::llvm_as_path`'s pattern (env var → PATH lookup, no
  fallback path).

## Todo (next)

- **[iostream]** `cout << char` regression fixture — verify it flows
  through the cstr probe and rule out future probe-collision regressions.
- **[iostream]** Probe more ostream overloads: `long`, `unsigned`,
  `bool`, `void*`.
- **[iostream]** `cin >> primitive` and `std::getline`. cin is a
  similar probe story to cout but the token-buffering on the awk side
  is non-trivial (~30 lines of `getline`-as-token-source helper).

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
- **[docs]** CHANGELOG.md once we cut a release.

## Probably won't do

- **[numerics]** i128 bignum — needs a full bignum runtime; cost
  outweighs use case for now.
- **[stdlib]** wide chars / `wstring` / locale facets — orthogonal
  rabbit hole, no clear win.

## Done (recent commits)

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
