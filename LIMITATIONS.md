# Limitations

Behavior gaps and approximations in the awk script awkvm produces.
Items here can fail (silently wrong output, wrong exit code, runtime
error) on otherwise valid C / C++. For deeper context on each, see
[ROADMAP.md](ROADMAP.md); for what's actively being worked on, see
[TODO.md](TODO.md).

## Numerics

- **`size_t`-style unsigned `<` / `>` comparison** — signed `icmp` is
  emitted in some cases, so values with the high bit set compare wrong.
  Pointer-typed unsigned compares were fixed in `f4a1fcd`; the broader
  case is still pending.
- **`__int128` / i128** — awk numbers are 64-bit doubles, so anything
  past 2^53 silently rounds. No fix without a bignum runtime.
- **IEEE 754 NaN, Inf, subnormals** — not preserved across mem load /
  store. Subnormals collapse to 0. NaN-vs-ordered FP comparisons
  collapse to the NaN-free answer.
- **Float arithmetic uses awk doubles** — `float` ops accumulate as
  double precision, then round only on `_store_f32`. Off-by-1-LSB on
  individual `fadd` / `fmul` results vs native float.

## Memory

- **`free` / `delete` are no-ops** — bump allocator never reclaims.
  Long-running programs grow `MEM[]` without bound and eventually
  exhaust gawk's memory.
- **64-bit ABI hardcoded** — pointer width 64 baked into runtime and
  codegen (`_load(p, 64)`, alloca alignment, argv stride).

## Control flow

- **Phi cycles at branch sites** — phi destinations resolved
  sequentially. The swap pattern (where two phis trade values in the
  same target block) emits wrong copies.

## C++ exceptions

- **`__cxa_end_catch` is a no-op** — destructors don't run on the
  caught exception object. Memory leaks, but otherwise OK for catches
  that don't depend on dtor side effects.
- **Multi / virtual inheritance RTTI not handled** — only Itanium
  `__si_class_type_info` (single inheritance) is walked. `catch
  (Base&)` only matches when `Base` is reachable through the SI chain
  on the thrown type.
- **MSVC EH** (`cleanuppad` / `catchpad`) bails. We only model the
  Itanium landingpad / resume model.

## C++ stdlib / iostream

- **`std::endl` doesn't write `\n`** — clang inlines `endl` into a
  virtual `widen('\n')` dispatch through the ctype facet's vtable,
  which we don't model. Workaround: write `"\n"` literal.
- **`ofstream` / `ifstream` / `stringstream` not supported** — the
  constructors emit no-op stubs; `<<` / `>>` against them currently
  fall through to stdout / read nothing.
- **`cout.rdbuf(...)` swap not tracked** — ostream identity is
  permanently bound to its initial output target in our model.
  Redirect idioms (`cout.rdbuf(captured.rdbuf())` for testing)
  silently won't work.
- **libc++ ABI tag drift** — see "Toolchain coupling" below. libc++
  tags templated helpers like `__put_character_sequence` with a
  per-version suffix (`B8ne190102` vs `B8ne190107` etc.); awkvm's
  probe pipeline pins to one specific tag at build time.
- **libstdc++ symbol names** — the cerr / clog dispatcher hardcodes
  libc++'s `_ZNSt3__14cerrE` / `_ZNSt3__14clogE`. Under libstdc++ the
  names are `_ZSt4cerr` / `_ZSt4clog`; output silently falls through
  to stdout.

## C++ stdlib / containers

- **`std::string` / `std::vector` not lifted** — stored byte-for-byte
  in `MEM[]` matching the libc++ layout. Operations walk MEM via the
  libc++ method calls, so string-heavy programs run slow. ADT lifting
  (D2-D6 in TODO) is the planned path to awk-native representation.

## Concurrency / atomics

- **`atomicrmw`** common ops handled (xchg / add / sub / and / or /
  xor / nand / smin / smax). `cmpxchg` and `fence` not wired.
- **TLS / `thread_local`** treated as flat global. Reads work in
  single-threaded code; multi-thread semantics don't apply.

## Platform

- **`memset_pattern{4,8,16}`** is Darwin-only. Linux clang doesn't
  emit these; the helpers in `runtime/libc.awk` are inert there.
- **Tested toolchain**: clang 19 + libc++ 19 + macOS arm64. Other
  combinations are best-effort (Linux verification pass on the
  ROADMAP).

## Toolchain coupling

`awkvm` bakes its `PROBE_MAP` at `cargo build` time using the
`clang++` at `$LLVM_SYS_191_PREFIX/bin/clang++` (Homebrew Clang 19
by default, set in `.cargo/config.toml`). The mangled names captured
there are valid **only for `.ll` files produced by the same
toolchain**. If you compile your C/C++ with a different clang, any
recognized stdlib symbol (most of `iostream`, anything else the
probe pipeline learns) falls through to a no-op stub.

Concretely on macOS: Apple Clang (`/usr/bin/clang`) ships its own
libc++ that tags `__put_character_sequence` with `B8ne190102`;
Homebrew Clang 19 tags it `B8ne190107`. `PROBE_MAP` only has the
latter, so a `.ll` from Apple Clang runs but `cout << "literal"`
produces no output. The mangled `<<` overloads for `int` / `long`
etc. don't carry an ABI tag, so those still print — leading to
mixed-output bugs that look like "spaces and newlines went missing"
rather than a clean failure.

The failure mode is **silent**: gawk exits cleanly with wrong /
missing output. Codegen doesn't currently detect the mismatch.

Workaround: drive your build through
`/opt/homebrew/opt/llvm@19/bin/clang++` explicitly. Don't rely on
`/usr/bin/clang` (or anything from PATH) unless you've verified
your system clang matches `$LLVM_SYS_191_PREFIX`. The shipped
`local/scripts/awkvmcc.sh` already pins to the right path; user
build scripts should do the same.

A proper fix would be a runtime sanity check at codegen time —
walk `module.func_declarations` for any `__put_character_sequence`
or other probe-targeted symbol and warn loudly if its full mangled
name (including the ABI tag) isn't in `PROBE_MAP`. On the TODO list.

## FFI

- **Multiple `awkvm --library` outputs can't be combined downstream.**
  `gawk -f libA.awk -f libB.awk` fails on duplicate `function _alloc`
  (and every other runtime helper); even if it didn't, both libraries
  re-run `BEGIN { NEXT_ADDR = 1 }` and re-allocate globals into the
  same `MEM[]`, aliasing each other. Combine `.bc` files via
  `llvm-link` *before* invoking awkvm so the runtime + globals exist
  in a single instance. See
  [`docs/awkvm-export.md`](docs/awkvm-export.md#combining-multiple-libraries).
- **`awkvm_export` ABI is primitive-only.** No pointer / struct
  marshaling at the boundary in v0.2.0; the type checker bails at
  codegen time. Inside a translated function, all the usual codegen
  applies — the limit is just at the export wrapper.

## Permanently out of scope

These would change what awkvm is, not just how much it covers:

- Real OS threads / signals / `fork` + `exec` / direct syscalls.
- Vector / SIMD types (x86 / NEON / AVX intrinsics).
- Inline assembly that's actual machine code (the inline-awk feature
  in ROADMAP is a different mechanism, not real asm).
- Multi-module dynamic linking; awkvm always emits one self-contained
  `.awk`.
- Wide-char / `wchar_t` / `wstring` / locale facets / multi-byte
  character handling (we pin `LC_ALL=C` for runs).
