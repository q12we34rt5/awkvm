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
#define AWKVM_BODY(s) __attribute__((annotate("awkvm_body" s)))

AWKVM_BODY(
    "(x, lo, hi):"
    "if (x < lo) return lo\n"
    "if (x > hi) return hi\n"
    "return x"
)
int clip(int x, int lo, int hi);
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
awkvm_body(arg1, arg2, ...): <awk body>
```

- The `(args)` list provides explicit awk parameter names. clang
  -O1 strips C-source parameter names from the IR (they become
  `%0`, `%1`, ...), so the rename keeps the body readable.
- Argument count must match the C function's signature.
- Bare form `awkvm_body:body` is also accepted; the awk body then
  references parameters by their IR names (`r0`, `r1`, ...). Useful
  for parameter-free functions; awkward otherwise.

## Multi-line bodies

Two ways:

**C-style adjacent string literal concatenation** (works in C and C++):
```c
AWKVM_BODY(
    "(x, lo, hi):"
    "if (x < lo) return lo\n"
    "if (x > hi) return hi\n"
    "return x"
)
```

The compiler joins the `"..."` tokens at parse time; `\n` becomes a
real newline in the resulting string. awkvm splits the body on `\n`
and re-indents each line in the emitted awk function.

**C++11+ raw string literal**:
```cpp
AWKVM_BODY(R"((x, lo, hi):
if (x < lo) return lo
if (x > hi) return hi
return x)")
```

`R"(...)"` doesn't interpret escapes, so literal newlines in the
source are preserved as-is.

## Declare-only vs full body

Two C-side patterns work:

**Declare-only** (awkvm-only target — no native build needed):

```c
AWKVM_BODY("(x, lo, hi):if (x < lo) return lo; if (x > hi) return hi; return x")
int clip(int x, int lo, int hi);
```

The C function has no body. awkvm picks up the annotation and emits
the awk function. Native compile would link-error against `clip` —
fine if you never compile this for native.

**Dual-build** (native and awkvm both target):

```c
AWKVM_BODY("(x, lo, hi):if (x < lo) return lo; if (x > hi) return hi; return x")
int clip(int x, int lo, int hi) {
    if (x < lo) return lo;
    if (x > hi) return hi;
    return x;
}
```

The C body runs in native compile; awkvm uses the annotation
instead. You're responsible for keeping the two implementations in
sync — handy for cross-checking awk against a known-good C
reference.

awkvm handles both — the only mechanical difference is whether the
function appears in `module.functions` (full body) or
`module.func_declarations` (declare-only).

## C++ callers

Same `extern "C"` requirement as `--link` helpers — see
[`docs/link-awk.md`](link-awk.md#c-callers-must-wrap-externs-in-extern-c).
In a `.cpp` file:

```cpp
extern "C" {
    AWKVM_BODY("(x, lo, hi):if (x < lo) return lo; if (x > hi) return hi; return x")
    int clip(int x, int lo, int hi);
}
```

Without `extern "C"`, clang++ mangles `clip` to `_Z4clipiii` and
the annotation references the mangled name; the codegen path still
works for both calling and emitting, but you'll be reading
`_Z4clipiii` in the generated awk instead of `clip`.

## See also

- [`examples/awkvm_body.c`](../examples/awkvm_body.c) — fixture
  exercised by `cargo test`. Uses argv inputs to stop clang from
  const-folding the `clip(...)` call.
- [`docs/inline-awk.md`](inline-awk.md) — statement-level inline
  awk (`__asm__("AWKVM:...")`) for cases where you want awk inside
  a mostly-C function instead of replacing the whole body.
- [`docs/link-awk.md`](link-awk.md) — pulling functions from a
  separate `.awk` file.
