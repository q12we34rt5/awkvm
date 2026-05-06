# TODO

Active backlog. Items move between sections as they progress.

For long-term direction (including stuff that's "wishlist" rather
than "next up"), see [ROADMAP.md](ROADMAP.md). For what's
user-visible-broken right now, see [LIMITATIONS.md](LIMITATIONS.md).

## In progress

_(empty — last item just landed)_

## Todo (next)

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
- **[docs]** CHANGELOG.md once we cut a release.

## Probably won't do

- **[numerics]** i128 bignum — needs a full bignum runtime; cost
  outweighs use case for now.
- **[stdlib]** wide chars / `wstring` / locale facets — orthogonal
  rabbit hole, no clear win.

## Done (recent commits)

- _(this commit)_ v0.1.0 release: README + CHANGELOG + stats_cli demo
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
