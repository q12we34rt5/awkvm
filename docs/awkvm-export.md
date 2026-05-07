# Exposing C functions to outside awk: `awkvm_export` + `--library`

The inverse of [`docs/link-awk.md`](link-awk.md). Compile a C
function with awkvm and call it from a hand-written awk script.

```sh
awkvm lib.bc --library -o lib.awk
gawk -f lib.awk -f script.awk    # script.awk calls the exported names
```

## Basic usage

```c
#define AWK_EXPORT __attribute__((annotate("awkvm_export")))

AWK_EXPORT int triangle(int n) {
    return n * (n + 1) / 2;
}
```

```awk
# script.awk
BEGIN { print triangle(10) }    # → 55
```

awkvm emits the function body under the usual `fn_<name>` convention
plus a thin bare-name wrapper that forwards into it:

```awk
function fn_triangle(r0,    r4, r5, r6) { ... }
function triangle(p0) { return fn_triangle(p0) }
```

The wrapper preserves the `fn_<name>` internal naming (so awkvm-side
callers and `--link`-imported helpers still match) and exposes the
C-source name verbatim (no `fn_` leakage into the awk-side API).

## `--library` mode

```sh
awkvm program.bc --library -o lib.awk
```

Drops the `BEGIN { exit fn_main(...) }` boot line so the script
doesn't auto-run on `gawk -f`. Use this whenever the file is meant
to be loaded as a library, with the actual entry point coming from
a separate `-f script.awk`.

`--library` and a missing `main` are independent: a `.bc` without
`main` already produces no boot line, but `--library` is still the
clearer signal for callers.

## Type restriction (v0.2.0)

Only primitive types cross the boundary:

| Allowed (param + return) | Notes |
| --- | --- |
| `int` / `long` / `unsigned` | i32 / i64 |
| `bool` | i1 |
| `char` | i8 |
| `double` / `float` | IEEE 754 |
| `void` | return only |

Pointers, structs, arrays, and aggregates bail at codegen with a
message pointing to a future marshaling layer:

```
awkvm_export: parameter 0 of `greet` is `i8*`. Only primitive types
(int / long / unsigned / double / bool / char) cross the v0.2.0
export boundary; pointer / struct support is deferred to a
follow-up marshaling layer.
```

If you need string I/O across the boundary today, do it through the
existing `_cstr` / `_str_to_mem` helpers from [`docs/inline-awk.md`](inline-awk.md)
or [`docs/link-awk.md`](link-awk.md) — both expose pointer-as-int8
addresses that the awk side can decode itself.

## C++ callers

Same `extern "C"` rule as the rest of the FFI surface — without it,
`awkvm_export` ends up annotating the mangled name (`_Z8triangle i`)
and the awk-side caller would have to use the mangled string. In a
`.cpp` file:

```cpp
extern "C" {
    AWK_EXPORT int triangle(int n) { return n * (n + 1) / 2; }
}
```

## Combining with `awkvm_fn`

Both annotations can coexist. The `awkvm_fn` body provides the
implementation, `awkvm_export` makes it externally callable:

```c
#define AWK_EXPORT __attribute__((annotate("awkvm_export")))
#define AWKVM_FN(decl, body) __attribute__((annotate("awkvm_fn" body))) decl;

AWK_EXPORT AWKVM_FN(
    int triangle(int n),
    "(n) { return n * (n + 1) / 2 }"
)
```

Order of the attributes doesn't matter; clang merges them onto the
same `@llvm.global.annotations` entry per function.

## See also

- [`examples/awkvm_export.c`](../examples/awkvm_export.c) — fixture
  exercised by `cargo test`. Three exports cover int / double / int
  signatures, called from
  [`examples/awkvm_export_caller.awk`](../examples/awkvm_export_caller.awk).
- [`docs/link-awk.md`](link-awk.md) — the inverse direction:
  hand-written awk callable from C via `extern`.
- [`docs/inline-awk.md`](inline-awk.md) — statement-level inline awk.
- [`docs/awkvm-fn.md`](awkvm-fn.md) — replacing a C function's body
  with a hand-written awk body.
