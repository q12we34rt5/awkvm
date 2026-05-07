# awkvm

Compile LLVM IR to a [gawk](https://www.gnu.org/software/gawk/) script.

awkvm consumes LLVM bitcode (`.bc`) or text IR (`.ll`) and emits a single
awk program that, when run with gawk, executes the original semantics.
The bet is that gawk already provides what a small VM needs — IEEE 754
arithmetic, associative arrays, recursive functions, and `system()` as
an escape hatch — so a hand-built bash runtime is unnecessary and a
single self-contained `.awk` file is enough to host most "ordinary"
C programs.

## Status

Integer / float arithmetic, control flow, bitwise ops, width
conversions, direct + indirect calls, a byte-addressed memory model
(alloca / load / store / GEP, with type punning), globals with
constant initializers, a libc bridge (printf / puts / putchar /
malloc / free / exit / memcpy / memset / memmove / strlen / atoi /
atof / ...), floats (single + double, IEEE 754 load/store), first-class
aggregates, C++ exceptions with single-inheritance RTTI matching, and
C++ stdlib smoke tests (`std::min`, `std::vector<int>`, `std::string`,
`std::vector<std::string>`, `std::any`) work today.

`iostream` is usable for primitive types via a probe-based binding
(`probes/`): `cout` / `cerr` / `clog` with `int` / `long` / `unsigned` /
`bool` / `void*` / `double` / `char` / `const char*`. `cin >>` reads
`int` / `long` / `double` with token-aware line buffering. `std::endl`,
`ofstream` / `ifstream`, `sstream`, `iomanip` aren't wired yet.

See [LIMITATIONS.md](LIMITATIONS.md) for current behaviour gaps,
[ROADMAP.md](ROADMAP.md) for direction, [CHANGELOG.md](CHANGELOG.md)
for release notes.

## Feature guides

`docs/` collects per-feature recipes. Today:

- [`docs/inline-awk.md`](docs/inline-awk.md) — `__asm__("AWKVM:...")`
  for dropping raw awk into C / C++. Lets you reach gawk's full
  surface (regex, subprocess pipes, file I/O, coprocess, time, ...)
  without us writing per-feature stubs.
- [`docs/link-awk.md`](docs/link-awk.md) — `awkvm --link helpers.awk`
  pulls a hand-written awk file into the emitted script; functions
  defined as `fn_<name>` become callable from C-side `extern` decls.
- [`docs/awkvm-fn.md`](docs/awkvm-fn.md) —
  `__attribute__((annotate("awkvm_fn(args) { body }")))` lets a C
  function carry its awk implementation in the annotation; awkvm
  uses the annotation as the body and skips IR translation.
- [`docs/awkvm-export.md`](docs/awkvm-export.md) — `awkvm --library`
  + `__attribute__((annotate("awkvm_export")))` expose C functions
  to outside awk callers under bare names. Call into the compiled
  library with `gawk -f lib.awk -f script.awk`. v0.2.0 export ABI
  is primitive-only (int / long / double / bool / char / void).
- [`docs/io.md`](docs/io.md) — file and stream I/O. v0.3.0
  unifies libc `FILE*` and C++ `<fstream>` on the same
  address-keyed stream subsystem. `fopen` / `fwrite` / `fread`
  / `system` / `popen` on the libc side; `ofstream` / `ifstream`
  with `<<` / `>>` / `read` / `write` / `gcount` on the C++
  side. Notes the `rdbuf`-swap limitation and what's deferred to
  v0.4.0 iostream completion.

## Build

LLVM 19 + gawk are required. On macOS (Apple Silicon):

```sh
brew install llvm@19 gawk
cargo build
cargo test    # 46 end-to-end fixtures + 5 awk-runtime unit-test bundles
```

`.cargo/config.toml` points `llvm-sys` at `/opt/homebrew/opt/llvm@19`.
For Intel Macs or Linux, edit that file to match your install prefix.

> **Toolchain coupling — important.** awkvm bakes a table of
> recognized libc++ mangled names at `cargo build` time using the
> `clang++` at `$LLVM_SYS_191_PREFIX/bin/clang++` (Homebrew Clang 19
> by default). Your input `.ll` **must** come from the same
> toolchain, or recognized stdlib calls (most of `iostream`) silently
> fall through to no-op stubs. Pin to
> `/opt/homebrew/opt/llvm@19/bin/clang++` rather than `/usr/bin/clang`
> when producing IR. awkvm warns at codegen time when it spots an
> ABI-tag mismatch. Full story:
> [LIMITATIONS.md "Toolchain coupling"](LIMITATIONS.md#toolchain-coupling).

## Quick example

`examples/cli/stats_cli.cpp` reads N then N integers, prints sum / min /
max / mean to stdout (and an error to stderr on bad input). End-to-end:

```sh
CLANGXX=/opt/homebrew/opt/llvm@19/bin/clang++
"$CLANGXX" -O1 -std=c++17 -emit-llvm -S examples/cli/stats_cli.cpp \
    -o /tmp/stats_cli.ll
./target/debug/awkvm /tmp/stats_cli.ll -o /tmp/stats_cli.awk

printf '5\n10 -3 7 0 8\n' | LC_ALL=C gawk -f /tmp/stats_cli.awk
# n=5 sum=22 min=-3 max=10 mean=4.4
```

Source: [`examples/cli/stats_cli.cpp`](examples/cli/stats_cli.cpp).

## Usage

```sh
awkvm program.bc -o program.awk
awkvm program.ll -o program.awk    # auto-converts via llvm-as
LC_ALL=C gawk -f program.awk
echo "exit=$?"
```

Without `-o`, the awk script is written to stdout. The generated
script requires **gawk** — it uses `and` / `or` / `xor` / `lshift` /
`rshift` built-ins for bitwise operations; POSIX awk and BSD/one-true
awk don't provide these. `LC_ALL=C` keeps gawk in single-byte mode so
UTF-8 escapes in the program's output reach the terminal verbatim
instead of being re-encoded byte-by-byte through the locale's
character handling.

## Generating fixtures

```sh
CLANGXX=/opt/homebrew/opt/llvm@19/bin/clang++
"$CLANGXX" -O1 -std=c++17 -emit-llvm -S examples/iostream/cppio.cpp -o examples/iostream/cppio.ll
"$CLANGXX" -O1 -std=c++17 -emit-llvm -c examples/iostream/cppio.cpp -o examples/iostream/cppio.bc
```

For `.c` sources, swap `clang++` for `clang` and drop `-std=c++17`.

Prefer `-O1` over `-O0`: optimised IR drops the per-variable
`alloca`/`load`/`store` boilerplate that clang otherwise emits, and
the resulting awk is much easier to read.

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. Unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in this work, as defined in the
Apache-2.0 license, shall be dual-licensed as above without any
additional terms or conditions.
