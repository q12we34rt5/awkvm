# awkvm

Compile LLVM IR to a [gawk](https://www.gnu.org/software/gawk/) script.

awkvm consumes LLVM bitcode (`.bc`) or text IR (`.ll`) and emits a single
awk program that, when run with gawk, executes the original semantics.
The bet is that gawk already provides what a small VM needs — IEEE 754
arithmetic, associative arrays, recursive functions, and `system()` as
an escape hatch — so a hand-built bash runtime is unnecessary and a
single self-contained `.awk` file is enough to host most "ordinary"
C programs.

> Work in progress. Integer arithmetic, control flow, bitwise ops,
> width conversions, direct + indirect calls, a byte-addressed memory
> model (alloca / load / store / GEP, with type punning), globals
> with constant initializers, a basic libc bridge (printf / puts /
> putchar / malloc / free / exit / memcpy / memset / memmove / strlen),
> floats (single + double, IEEE 754 load/store), first-class aggregates,
> C++ exceptions with single-inheritance RTTI matching, and small
> C++ stdlib smoke tests (`std::min`, `std::vector<int>`,
> `std::string`, `std::vector<std::string>`) work today. iostream is
> out of reach because the implementation lives in libc++ dylib, not
> the IR.

## Build

LLVM 19 is required at build time. On macOS (Apple Silicon):

```sh
brew install llvm@19
cargo build
```

`.cargo/config.toml` points `llvm-sys` at `/opt/homebrew/opt/llvm@19`.
For Intel Macs or Linux, edit that file to match your install prefix.

## Usage

```sh
awkvm program.bc -o program.awk
awkvm program.ll -o program.awk    # auto-converts via llvm-as
gawk -f program.awk; echo $?
```

Without `-o`, the awk script is written to stdout.

The generated script requires **gawk** (`brew install gawk` on macOS).
It uses gawk's `and`/`or`/`xor`/`lshift`/`rshift` built-ins for bitwise
operations; POSIX awk and BSD/one-true awk don't provide these.

## Generating fixtures

```sh
clang -O1 -emit-llvm -S examples/add.c -o examples/add.ll
clang -O1 -emit-llvm -c examples/add.c -o examples/add.bc
```

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
