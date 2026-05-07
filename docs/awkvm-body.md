# Whole-function awk body via `__attribute__((annotate))`

Replace a C function's body with a hand-written awk body, while
keeping the C type signature for callers. awkvm reads the
`awkvm_body` annotation and emits the awk verbatim, skipping IR
translation for that function.

Use this when:
- The C type signature is what callers see, but the natural
  implementation is awk (regex, gawk built-ins, awk associative
  arrays, ...).
- You want a thin wrapper around something inline awk would handle
  but cleaner-organized as its own function.

For embedding raw awk *inside* an otherwise IR-translated function,
use [`docs/inline-awk.md`](inline-awk.md). For pulling in a
hand-written `.awk` library file, use
[`docs/link-awk.md`](link-awk.md).

## Basic usage

```c
#define AWKVM_FN(decl, body) __attribute__((annotate("awkvm_body" body))) decl;

AWKVM_FN(
    int clip(int x, int lo, int hi),
    "(x, lo, hi) {"
    "    if (x < lo) return lo\n"
    "    if (x > hi) return hi\n"
    "    return x\n"
    "}"
)
```

awkvm emits:

```awk
function fn_clip(x, lo, hi) {
    if (x < lo) return lo
    if (x > hi) return hi
    return x
}
```

C-side callers go through the standard `fn_<name>` codegen path —
both `clip(input, 0, 100)` from C and `fn_clip(...)` from inline
awk reach the body.

## Annotation form

```
awkvm_body(arg1, arg2, ...) { <awk body> }
```

- The `(args)` list provides the awk function's parameter names.
  clang -O1 strips C-source parameter names from the IR, so
  explicit naming here keeps the body readable and avoids a silent
  name-mismatch.
- Argument count must match the C function's signature; mismatch
  bails at codegen with a clear message.
- The `{ ... }` braces wrap the body. awkvm strips the outer pair,
  trims leading and trailing blank lines, and dedents the common
  leading whitespace so the emitted output stays cleanly indented.
- The visual structure mirrors a normal awk function definition,
  which is the point — the macro hides the annotation plumbing.

## The `AWKVM_FN` macro

```c
#define AWKVM_FN(decl, body) __attribute__((annotate("awkvm_body" body))) decl;
```

Two arguments: the C declaration and the awk body string. The macro
appends `;` automatically so the user-side declaration doesn't carry
a trailing semicolon inside the macro args. Works for both
declare-only (`int clip(int, int, int)`) and dual-build
(`int clip(int x, int lo, int hi) { /* C body */ }`); in the latter
case the trailing `;` becomes a stray empty declaration, which
clang accepts silently.

## Multi-line bodies

The body is a single C-string-literal expression, so any C string
trick that produces multiple lines works.

**C-style adjacent string literal concatenation** (works in C and C++):

```c
AWKVM_FN(
    int clip(int x, int lo, int hi),
    "(x, lo, hi) {"
    "    if (x < lo) return lo\n"
    "    if (x > hi) return hi\n"
    "    return x\n"
    "}"
)
```

The compiler joins the `"..."` tokens at parse time; `\n` becomes a
real newline in the resulting string.

**C++11+ raw string literal** (no escape interpretation, literal
newlines preserved):

```cpp
AWKVM_FN(
    int clip(int x, int lo, int hi),
    R"((x, lo, hi) {
        if (x < lo) return lo
        if (x > hi) return hi
        return x
    })"
)
```

## Declare-only vs dual-build

Two C-side patterns both work:

**Declare-only** (awkvm-only target):

```c
AWKVM_FN(int clip(int x, int lo, int hi),
         "(x, lo, hi) { ... }")
```

The C function has no body. awkvm picks up the annotation and emits
the awk function. Native compile would link-error against `clip` —
fine if you never compile this for native. The `AWKVM_FN` macro's
trailing `;` makes this a plain declaration.

**Dual-build** (native and awkvm both target):

```c
AWKVM_FN(
    int clip(int x, int lo, int hi) {
        if (x < lo) return lo;
        if (x > hi) return hi;
        return x;
    },
    "(x, lo, hi) { ... }"
)
```

The C body runs in native compile; awkvm uses the annotation
instead. You're responsible for keeping the two implementations in
sync — handy for cross-checking awk against a known-good C
reference. The macro-appended `;` after the function definition is
a stray empty declaration, harmless.

awkvm picks up annotations on both shapes (`module.functions` for
full body, `module.func_declarations` for declare-only).

## C++ callers

Same `extern "C"` requirement as `--link` helpers — see
[`docs/link-awk.md`](link-awk.md#c-callers-must-wrap-externs-in-extern-c).
In a `.cpp` file:

```cpp
extern "C" {
    AWKVM_FN(int clip(int x, int lo, int hi),
             "(x, lo, hi) { ... }")
}
```

Without `extern "C"`, clang++ mangles `clip` to `_Z4clipiii` and
the annotation references the mangled name; calling and emitting
still work, but you'll be reading `_Z4clipiii` in the generated awk
instead of `clip`.

## See also

- [`examples/awkvm_body.c`](../examples/awkvm_body.c) — fixture
  exercised by `cargo test`. Uses argv inputs to stop clang from
  const-folding the `clip(...)` call.
- [`docs/inline-awk.md`](inline-awk.md) — statement-level inline
  awk (`__asm__("AWKVM:...")`) for cases where you want awk inside
  a mostly-C function instead of replacing the whole body.
- [`docs/link-awk.md`](link-awk.md) — pulling functions from a
  separate `.awk` file.
