# Roadmap

awkvm's stated target is **running C++ programs**, including `try` / `catch`.
C is the milestone before that. Most C++ features (classes, vtables,
templates, ctor/dtor, inheritance, operator overloading) are lowered to
ordinary IR by clang — so the C++-specific work concentrates in indirect
calls + exception mechanics + RTTI matching, not the language itself.

Phase commits land with a fixture that exits with a known value. Items
outside the numbered phases are itemised by theme below.

## Done

- **Phase 1** — Integer arithmetic, ret, direct calls. `add.c`
- **Phase 2** — Control flow: icmp, br, condbr, phi. `sum.c`
- **Phase 3** — Bitwise ops, width conversions, select, switch. `bits.c`
- **Phase 4** — Byte-addressed memory: alloca, load, store, GEP, ptr casts; type punning works. `point.c`, `buf.c`
- **Phase 5** — Globals with constant initializers (Int / Array / Struct / GlobalReference). `table.c`, `str.c`
- **Phase 6** — libc bridge: printf / puts / putchar / malloc / free / mem* / exit. `hello.c`
- **Phase 7** — Indirect calls via id-keyed dispatcher. `fnptr.c`
- **Phase 8** — Floats (single + double, IEEE 754 load/store). `floats.c`
- **Phase 9** — First-class aggregates (extractvalue / insertvalue). `agg.c`
- **Phase 10** — C++ exception control flow + minimal `__cxa_*` runtime. `throw_int.cpp`
- **Phase 11** — Single-inheritance RTTI matching (`catch (Base&)` ← `throw Derived`). `throw_class.cpp`
- **Phase 12** — C++ stdlib smoke: `std::min`, `std::vector<int>`, `std::string`, `std::vector<std::string>`, `std::any`. Plus a real ~500-line C++ project (Q3Engine `plane_cube_shadow`) that renders byte-identical to native, exercising vtables, atomicrmw ref counts, virtual call dispatch, perspective math, Phong shading.
- **`main(int argc, char**)`** entry packs gawk's ARGV into a C-style char**; supports flag-driven CLI through libc bridge (`strcmp` / `atoi` / `atof` / `strtol` / `strtod`) and `_printf` `%f`-family conversions.

---

## Near-term (easy + clear value)

Ordered roughly by ratio of impact to effort.

- **Inline awk (the symmetry trick).** ~half-day. C/C++ already has inline
  asm; awkvm's "machine code" is awk, so the analogue writes itself. Today
  we bail on `Either::Left(InlineAssembly)` in `emit_call`. Switch that to:
  if the asm string starts with a sentinel prefix (e.g. `AWKVM:`), strip
  the prefix and emit the rest as raw awk, with `$N` placeholders
  substituted from the call operands.

  ```c
  #define awkvm_inline(...) __asm__(__VA_ARGS__)

  int x = 42, y;
  awkvm_inline("AWKVM:%0 = %1 * 2" : "=r"(y) : "r"(x));
  ```

  Lowers to `r_y = r_x * 2`. Suddenly the entire gawk surface — `system()`,
  `match(/regex/)`, `mktime()`, `print > "file"`, `cmd | getline x`,
  bidirectional `|&` — is reachable from C/C++ without ever adding a
  matching `emit_libc` stub. Several items lower in this roadmap (popen,
  chrono, regex, even some signal-style behaviour) become user-space
  problems that don't need awkvm changes at all. Like inline asm, the
  user takes on platform-awareness: a build with `__AWKVM__` defined uses
  the inline awk; native builds need their own implementation behind the
  same macro.

- **Whole-function awk bodies via `__attribute__((annotate))`.** ~half-day.
  Same surface area as inline awk but for entire function bodies. clang
  preserves `annotate` as `@llvm.global.annotations`; awkvm scans the array
  for entries prefixed `awkvm_fn`, builds a `name -> awk_source` map,
  and in `emit_function` short-circuits the IR translation when the
  function name is in the map.

  ```c
  #ifdef __AWKVM__
    #define AWK_FUNCTION(body) __attribute__((annotate("awkvm_fn" body)))
  #else
    #define AWK_FUNCTION(body)
  #endif

  extern "C" AWK_FUNCTION("return arg0 * 2") int awk_double(int arg0);
  ```

  Lowers to a verbatim `function fn_awk_double(arg0) { return arg0 * 2 }`
  in the output. Combines with the dual-build pattern: `#ifdef __AWKVM__`
  uses the annotation, native uses a real C body.

- **`--link helpers.awk`.** ~half-day. Different shape from the in-source
  embedding: a separate `.awk` file is concatenated into the output,
  defining functions C-side declares as `extern`. The coupling point is
  the function signature, not annotation strings; awk syntax highlighting
  / formatter / linter can do their job. Less convenient for dual-build
  (no shared source), more convenient for "I have 200 lines of awk
  helpers and I want them in a real .awk file".

- **`system()` libc intercept.** ~30 lines. gawk's `system(s)` already does
  blocking fork+exec. Mapping C's standard `int system(const char*)` is a
  one-liner in `emit_libc`. Unlocks "shell out from inside awkvm-compiled
  C/C++ code". (Subsumed by inline awk for power users, but standalone
  intercept is what most code expects.)
- **`fopen` / `fwrite` / `fclose` (write-only path).** Half-day. gawk's
  `print x > "file"` + `close("file")` already provides the underlying
  capability. Wrap as fd-keyed table; bytes go through `printf "%c"` per
  byte. Unlocks BMP write, log files, anything output-side.
- **`popen()` / `pclose()`.** Half-day. Same shape as fopen but the path
  is `cmd | getline` (read mode) or `print | cmd` (write mode). Standard
  POSIX C API; user code doesn't know it's awk.
- **Coroutine empirical verification.** 5 minutes. clang's `-O1` runs the
  coroutine lowering passes (`CoroEarly` / `CoroSplit` / `CoroElide` /
  `CoroCleanup`), so the IR we receive should already be flat — frame
  on heap via `operator new`, suspends become indirect calls. Write a
  30-line `co_yield 1, 2, 3` generator, grep the .ll for any leftover
  `llvm.coro.*` intrinsics; if clean, declare done.
- **Linux verification pass.** Half to one day. The codegen is
  ABI-agnostic in principle but only macOS/AArch64 has been driven hard.
  Compile the existing fixtures on Linux/x86_64 and compare. Expected
  outcome: most pass directly; a handful surface intrinsics or ABI
  patterns we haven't seen (Linux clang doesn't emit `memset_pattern16`,
  for instance, which we already special-cased for Darwin).
- **`fread`-style binary read.** Doable but uglier than fwrite. gawk's
  `getline` is line-based; we'd buffer line-by-line and slice into the
  caller's buffer. Worth doing only when a fixture demands it.

## Medium-effort directions

- **Full `<chrono>` / sleep / time.** gawk has `systime()` (epoch seconds)
  and `strftime()`; sleep can shell out (`system("sleep N")`) or use
  gawk's `time` extension. Bind `time()` / `gettimeofday()` /
  `nanosleep()`. ~half-day.
- **`<regex>` via gawk regex.** gawk has POSIX ERE built in. `match()` and
  `gsub()` could back `std::regex_search` / `regex_replace`. Surface API
  is templated header-only mostly, so the bridge is small.
- **`<random>` (predictable).** `std::random_device` won't work (libc++
  dylib), but `std::mt19937` is header-only template. Today the seed
  problem is "needs rd()". Workaround: provide a fixed seed surface
  (`__awkvm_seed()` via gawk's `srand()` / `rand()`), expose to user
  code. ~half-day.
- **`scanf` family.** Once `_atoi` / `_atof` / cstring stuff is wired,
  `sscanf` is a small parser on top. `fscanf` reads from a stream
  through the same `getline` plumbing as fread. ~one day.
- **awkvmcc driver.** Separate binary that wraps `clang -emit-llvm` +
  `llvm-link` + `awkvm`, accepting `.c` / `.cpp` directly and any
  number of files. Standard cc-style flag passthrough (`-D`, `-I`,
  `-O`). Stays out of awkvm's core (which keeps "single IR module in,
  single awk out" as its job). ~one day.
- **Bidirectional coprocess as awkvm extension.** Custom non-standard C
  API (`awkvm_coproc_open` / `_send` / `_recv` / `_close`) backed by
  gawk's `cmd |& getline` and `print |& cmd`. Useful for building
  "awk drives orchestration, C does compute" pipelines. Need to design
  the surface carefully since user code becomes awkvm-specific at this
  boundary.
- **Multi-inheritance / virtual inheritance RTTI.** `__vmi_class_type_info`
  is a variable-length record with a base array. Walk it during
  `_typeid_for`. ~half-day if the base offsets are simple, more if
  virtual base pointer adjustment needs modelling.
- **Real destructor sequencing in catch.** `__cxa_throw` already takes a
  `dtor` pointer; instead of ignoring it, store and call when the
  catch block exits. Then add real ref-count semantics to
  `__cxa_begin_catch` / `__cxa_end_catch`. ~half-day.
- **POSIX awk fallback (`--posix`).** Replace gawk's `and` / `or` / `xor`
  / `lshift` / `rshift` with per-bit awk loops. ~2 hours to implement.
  Cost is a 10–50× slowdown on bit-heavy programs (vtables, RGBA
  packing). Worth doing only if a target environment specifically
  forbids gawk.

## Big-ticket / research-grade

These are 質變 (qualitative-leap) projects; each likely a separate
sub-effort the size of half the existing work.

- **iostream / fstream / sstream via libc++ bitcode** (deferred —
  current work is via the probe pipeline instead, see TODO.md).
  Original plan: compile libc++ to bitcode (`clang++ -emit-llvm -c`
  over the libc++ source tree), link with the user's `.bc` via
  `llvm-link`. The probe-based path (build.rs runs probes, captures
  mangled names, codegen rewrites recognized calls to awk templates)
  has lower setup cost and crucially handles libc++ ABI drift
  automatically — at the price of writing one awk helper per binding.
  cout / cerr / clog and primitive `<<` overloads land first; sstream
  / fstream + `rdbuf()` swap come together as one streambuf-indirection
  unit later.
- **Real f32 precision.** Today every float arithmetic op is awk
  double-precision. To match native exactly we'd round each `fadd` /
  `fmul` / etc. when the IR type is float, by passing through
  `_f32_to_bits` + `_f32_from_bits`. Many helpers, real perf cost.
  Off-by-1-LSB on color channels is the visible symptom today.
- **Cooperative threads.** Single-core fake threading where `std::thread`
  becomes a coroutine, `mutex::lock` is a yield point, and a small
  scheduler picks the next ready coroutine. Doesn't give real
  concurrency but lets thread-using programs compile and run
  deterministically. Designs from green-thread runtimes (Erlang, Go's
  early days) apply.
- **Atomics with stronger semantics.** `cmpxchg`, `fence`, full memory
  ordering. Single-threaded so semantically trivial, but the IR shapes
  haven't been wired. Needed for any `std::atomic<T>` beyond the
  shared_ptr ref-count case we already handle.
- **Bignum / i128.** Two doubles or a digit-array per i128 value, with
  helper functions for every arithmetic op. Pervasive change in
  `_load` / `_store` / `_zext` / `_trunc`. Heavy. Useful if anyone
  wants 128-bit hash code or crypto primitives.
- **Vector / SIMD types.** Model `<4 x float>` as a 4-slot awk array,
  open up `extractelement` / `insertelement` / `shufflevector`.
  Affects load/store size calculations, value indexing, auto-vectorised
  loops. Many entry points but each is small.
- **Self-hosting curiosity.** Compile awkvm itself (or a rewrite in C)
  through awkvm. The Rust source uses `llvm-sys` so won't go through;
  a hypothetical pure-C reimplementation could. Mostly a stunt.

## Tooling and ergonomics

- **Source maps from awk → C/C++ line.** Emit `# {file}:{line}` comments
  per IR debug-loc, so reading the `.awk` is navigable when something
  goes wrong. The IR carries `!dbg` annotations; we currently ignore.
- **`awkvm run foo.cpp`.** Compile + execute in one shell command.
  Wraps the whole pipeline. Combines well with `awkvmcc`.
- **`awkvm trace`.** Emit awk that logs every basic-block transition
  to stderr, so debugging a misbehaving fixture doesn't need
  hand-inserted prints.
- **Step debugger.** gawk has `-D` interactive debug mode. With source
  maps + a basic block label table, breakpoints in user-source-line
  terms become possible.
- **`awkvm bench`.** Run a micro-benchmark suite and print
  awkvm/native speed ratios. Useful for tracking regression / progress
  on the dispatcher / inlining work.
- **REPL.** An interactive mode that compiles a snippet + runs it, for
  exploring IR shapes. Probably more cute than useful.
- **Better error messages.** Today `bail!("instruction not implemented:
  {other}")` is the standard. Adding the function name / debug-loc
  context would help users reading the error.

## Performance

The status quo runs ~100–1000× slower than native depending on the
shape of the program. Most of the slowdown is in dispatch and memory
ops, not arithmetic.

- **Dispatcher: gawk `switch`.** Today's `if (block == "b1") ... else
  if (block == "b2") ...` is O(blocks) per dispatch. gawk's
  `switch (block)` compiles to a faster lookup. Same for `_icall`'s
  function-id chain.
- **Dispatcher: numeric block ids.** Use integer block ids instead of
  string labels. `if (block == 1)` is slightly faster than
  `if (block == "b1")`.
- **`_icall`: function-table.** Build a `FN[id] = "fn_name"` table at
  init, dispatch via gawk's `@FN[id](args)` indirect call. O(1)
  dispatch, no chain.
- **alloca lifetime tracking.** stack-style allocation for alloca
  values that don't escape, releasing on function return. Reduces MEM
  growth vs the bump allocator's monotonic.
- **Inline small alloca.** A scalar-only alloca that never has its
  address taken can be a direct awk variable, no MEM at all.
- **Constant folding in codegen.** Many constexprs we currently leave
  as runtime arithmetic could be precomputed. Cheap win on cold
  startup.
- **memcpy fast paths.** For memcpy(dst, src, N) with known constant
  N, unroll instead of loop. Big for vtable-heavy code.
- **Real GC.** Mark-and-sweep over MEM with a small set of roots
  (live SSA registers in the current call stack). Long-running
  programs leak today.

## Coverage / correctness

- **Phi cycles.** Sequential resolution of phis at branch sites is
  wrong when a phi destination overlaps an incoming source from
  another phi. Fix: temp-based parallel copy.
- **Unaligned access.** We don't enforce alignment on `_load` / `_store`,
  which happens to match the target's tolerance; if anyone ever runs
  awkvm on a strict-alignment target this might surface.
- **Constant aggregates as operands.** `extractvalue` / `insertvalue`
  with a `Constant::Struct` / `Constant::Array` operand (rather than
  an `undef` / `aggregatezero` start) isn't materialised today.
  Would land if a fixture exercises it.
- **`extractvalue` / `insertvalue` of nested aggregate operands.**
  Same root cause; the recursion path exists but isn't covered.
- **MS-style EH (`cleanuppad` / `catchpad`).** Today only Itanium
  landingpad/resume is wired. MSVC ABI uses funclets, different
  control-flow shape. If anyone targets `pc-windows-msvc` it'll bail.
- **`setjmp` / `longjmp`.** Currently bail. Could be implemented since
  we control the stack model — `longjmp` would walk back through
  awk's call stack via an exception-style global flag, similar to
  `UNWINDING`.
- **TLS / thread-local storage.** Treat as flat global today (since
  we're single-threaded). Works for `thread_local` reading; doesn't
  match real semantics if anyone tries multi-thread.
- **Inline asm (real asm, not awkvm's inline-awk).** Bail. Conceivable to
  recognise a few common patterns (`rdtsc` returning fixed garbage,
  `cpuid` returning something reasonable) but mostly out of scope. The
  inline-awk feature in Near-term shares the IR shape but uses a
  sentinel prefix to disambiguate.
- **`printf` precision / parameterised width.** `%.5d`, `%*d`, `%-10s`
  partially work because we forward the spec to gawk's printf, but
  edge cases (especially negative width via arg) haven't been tested.
- **Locale / multi-byte.** We pin `LC_ALL=C` for the run script. Real
  multi-byte support (UTF-8 string handling, wchar_t) is beyond.

## Engineering / code quality

Honest internal review. None of this is user-facing; all of it matters
if anyone other than the original author tries to read or extend
codegen.rs, or if we ever try to ship this as a serious crate.

### Structure

- ✓ **codegen.rs split into per-concern submodules** (`ed9a15f`).
  Now 7 files under `src/codegen/`: `mod.rs`, `names.rs`, `types.rs`,
  `globals.rs`, `func.rs`, `mem.rs`, `call.rs`. Largest is `func.rs`
  (~500 lines, the `emit_instruction` switch + arithmetic helpers).
- ✓ **awk RUNTIME pulled out into `src/runtime/*.awk`** (`94c8b79`).
  Concatenated via `include_str!` in `src/runtime/mod.rs`. libc /
  Itanium-ABI helpers further split into `libc.awk` (`122cc0b`) so
  user-shadowed names get filtered out automatically.

### Boilerplate

- **`let _ = writeln!(out, "...")` is the dominant pattern.** Most
  emission code reads as walls of these calls. Wrap in a small
  `Emitter` struct with `assign(dest, expr)`, `store(addr, val, bits)`,
  `raw(line)` methods — same surface, less syntactic noise.
- **Repeated case splits.** `emit_load_at` / `emit_store_at` /
  `emit_atomicrmw` / `emit_extractvalue` / `emit_insertvalue` all
  case-split on type to choose `_load_f32` / `_load_f64` / `_load`
  with different micro-variations. Extract `load_helper_for(ty)` /
  `store_helper_for(ty)` once.

### Documented invariants

- **Integer encoding invariant is undocumented anywhere.** awkvm holds
  every integer SSA value as a sign-extended awk number: signed `i32` =
  `-1` and unsigned `i32` = `0xFFFFFFFF` are *the same awk value*
  (`-1`). This drives every choice around `_zext`, no-op `SExt`,
  `_and` / `_or` / `_xor` wrappers, and unsigned `icmp`. Should be a
  doc-comment block at the top of `lib.rs` so anyone reading codegen
  has it framed.
- **Pointer width assumption (64-bit) is hardcoded in many places.**
  `mem_bits` for `PointerType` returns 64; `type_size_bytes` returns
  8; argv builder uses `i * 8`; vtable slot math assumes `* 8`.
  Define `const PTR_BITS: u32 = 64;` and `const PTR_BYTES: u64 = 8;`
  once and reference them everywhere — would let us swap to a 32-bit
  pointer model with a single edit.

### Magic numbers

- **IEEE 754 constants in `runtime.awk`-equivalent are bare literals.**
  `2147483648` (2^31, sign bit), `8388608` (2^23, mantissa width),
  `4503599627370496` (2^52). No comment, no derivation. Either
  rewrite as `2^31` arithmetic (gawk evaluates at parse time) or add
  named constants in BEGIN: `BEGIN { F32_SIGN = 2^31; F32_MANT = 2^23; ...}`.

### Error messages

- Today: `bail!("instruction not implemented: {other}")`,
  `bail!("expected integer type, got {other}")`. No context.
- Use `anyhow::Context::with_context` to wrap at every `emit_function`
  / `emit_instruction` boundary so failures land as
  `"in function _Z..., block b3, instruction extractvalue: ..."`
  rather than just the leaf message.

### Tests

- ✓ **awk-side unit tests for runtime helpers** (`4b8e211`). Each
  `runtime/*.awk` has a matching `tests/runtime/*_test.awk`; the
  cargo integration test in `tests/runtime.rs` writes the shipped
  RUNTIME constant to a tempfile and drives gawk over it. Covers
  bitwise / mem / str / fp / cxa with 77 assertions. _f32 / _f64
  round-trip and sign-bit boundary cases are in `fp_test.awk`.
- ✓ **`cargo test` integration for fixtures** (`20d708c`).
  `tests/examples.rs` compiles each `examples/*.c{,pp}` to .ll via
  clang, runs awkvm, runs gawk, asserts `(exit, stdout, stderr)`
  against hardcoded expectations. ~25 fixtures, runs in ~0.5s.

### CI / repeatability

- **No CI** at all. A GitHub Actions workflow running `cargo build`,
  `cargo clippy`, and the fixture suite on macOS + Linux would catch
  90% of regressions for free.
- ✓ **`examples/*.ll` regeneration handled** (`20d708c`).
  `tests/examples.rs` compiles each fixture .c/.cpp to .ll on demand
  via clang at test time, so a fresh clone runs the full suite with
  just `cargo test`. Manual `examples/*.ll` files are still gitignored
  for ad-hoc experimentation but no longer required.
- **No CHANGELOG.md.** The git log is per-phase clean, but
  user-visible "what changed in v0.2 vs v0.3" doesn't exist. Becomes
  important if we ever publish to crates.io.

### clippy

- 3 warnings total today, all `manual_div_ceil` (cosmetic). Run
  `cargo clippy --fix` to clear.

## Distribution / project polish

- **Multi-LLVM-version support.** Today hard-coded against LLVM 19 via
  `llvm-sys`. Cargo features `llvm-18` / `llvm-19` / `llvm-20` plus
  conditional `cfg`s would let users build against whatever they have.
- **`cargo test` integration.** Today the examples are run by ad-hoc
  shell loops. Wrapping each fixture as a Rust integration test
  enables CI.
- **GitHub Actions CI.** Matrix over (macOS / Linux), gawk versions,
  LLVM versions. Catches regressions in any one combination.
- **Pre-built binaries.** GitHub release artifacts. Especially helpful
  on Linux where the `llvm-sys` dependency is non-trivial to satisfy.
- **`brew install awkvm`.** Once stable, a tap.
- **`crates.io` publication.** Once the API is settled.
- **Docker image.** Bundles LLVM 19 + gawk + awkvm. Lets people try
  without installing the toolchain.
- **Documentation site.** Per-IR-instruction reference: which awk
  pattern it lowers to, which gawk features it depends on, which
  intrinsics are supported, which aren't.
- **Example gallery.** A page of "C/C++ programs that compile through
  awkvm", with the awk output linked. Marketing more than engineering,
  but a clear way to communicate scope.
- **License files** ✓ already done (MIT + Apache-2.0).

## Punch list (small, no phase needed)

- Unsigned icmp ✓ (done)
- Phi cycles — still uses sequential resolution.
- atomicrmw ✓ (done for the common ops)
- cmpxchg / fence — not yet.
- declare-only stub call: print a one-time warning to stderr the first
  time each unimplemented stub is invoked, instead of silently
  no-oping. Helps diagnose silent-wrong-result bugs.
- Const-folded float arithmetic in initializers (e.g.
  `Constant::FAdd`) — not yet handled in `emit_const_init`.
- Fold insertvalue chains starting from `undef` into a single alloc
  + sequential stores, instead of N round-trips through memcpy.

## Far speculation

Listed without endorsement.

- **Self-hosting in pure C.** Rewrite awkvm in C, compile through
  awkvm itself, run awk-version-of-awkvm on a C program. The mother
  of all stunts.

- **Reverse direction: awkvm-output-awk → LLVM IR.** Partial inverse
  of the main pipeline. The awk we emit is regular enough (fixed
  `r<N>` naming, `_load` / `_store` runtime helpers carrying bit
  width, fixed multi-block dispatcher pattern) that a pattern-driven
  reverser is feasible. *Not* a general awk → IR compiler: only
  recognises shapes awkvm itself emits.

  What's mechanically recoverable: control flow, memory ops, calls,
  bitwise widths, basic block structure, function boundaries (when
  not inlined).

  What's lost: phi nodes (we spill to sequential assigns at branch
  sites), local variable names, debug info, comments, struct layout
  (recoverable only via alias analysis on `_load`/`_store` offset
  patterns), inlined function boundaries, anything constant-folded
  by `clang -O1` before we saw it.

  Killer demo: round-trip test. `foo.cpp -> clang -> foo.ll(orig) ->
  awkvm -> foo.awk -> reverser -> foo.ll'`, then `diff` to quantify
  exactly what we drop. Validates correctness, measures information
  loss, and closes a satisfying loop.

  Effort: 1–2 weeks for `add.c`-tier fixtures, 1–2 months to
  round-trip plane_cube_shadow. A perfect bidirectional pipeline
  isn't possible — information IS lost — but the recovered IR
  should pass `clang -x ir` and run as a native binary, which would
  be its own party trick.
- **Other transpilation targets.** awk is one. bash, sh, Tcl,
  PowerShell, MetaPost, even pure POSIX shell — same architecture,
  different backend. Compile-to-bash specifically would have its own
  pitch (hello, embedded systems with only `/bin/sh`).
- **Reverse: awk → C.** A different project entirely; lift dynamic
  awk back into typed C. AOT awk compilation.
- **WebAssembly target.** Compile WebAssembly modules to awk. Would
  let you take a Rust crate, compile to wasm, then awk-ify. Pipeline
  is more layers but each layer is well-defined.
- **Sandbox pitch.** gawk has limited capabilities by default
  (no arbitrary syscalls, no FFI, file I/O routes through known
  primitives). Generated awk inherits these limits — a sort of
  natural sandbox for untrusted C/C++.
- **Education pitch.** A tool that takes user C source, shows the IR,
  shows the awk, lets you run any of three. Useful for compiler
  courses; the awk is simple enough to read end-to-end.
- **Audit-trail pitch.** awk is human-readable. Reviewing what your
  C++ "actually does" is easier in awk than in `objdump`.

## Out of scope

Items that would not just be expensive but would change what awkvm
is. Listed for clarity, not as "we'll do them later".

- Real concurrency (true OS threads with kernel scheduling).
- Real signal handling (`sigaction`, `kill`, `raise`).
- True forking (a process tree below us).
- Direct syscalls bypassing libc.
- DLL / shared library production. We emit a single `.awk`.
- Multi-module dynamic linking with exported symbols.
- Hardware-specific instruction sets (x86 SIMD, NEON, AVX-512).
- Anything that needs precise time semantics (real-time loops).
