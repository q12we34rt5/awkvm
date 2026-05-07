# Linked awk helpers (`awkvm --link`)

Concatenate a hand-written `.awk` file into the emitted script and
expose its functions to C-side `extern` declarations. Useful when an
awk function is more than a one-liner — you get real `.awk` syntax
highlighting, formatter support, and gawk linting that inline awk
strings don't.

## Basic usage

`helpers.awk`:

```awk
function fn_clip(x, lo, hi) {
    if (x < lo) return lo
    if (x > hi) return hi
    return x
}
```

`prog.c`:

```c
extern int clip(int x, int lo, int hi);

int main(void) {
    int y = clip(input, 0, 100);
    /* ... */
}
```

Compile:

```sh
$LLVM_SYS_191_PREFIX/bin/clang -O1 -emit-llvm -S prog.c -o prog.ll
awkvm prog.ll --link helpers.awk -o prog.awk
gawk -f prog.awk
```

awkvm pastes `helpers.awk` verbatim into the output (right after
runtime, before user functions), and suppresses the no-op stub it
would otherwise emit for the unresolved `clip` symbol — so the C
call to `clip(5, 0, 10)` in IR routes through `fn_clip(5, 0, 10)`
into your hand-written awk body.

`--link` accepts multiple files: `awkvm prog.ll --link a.awk --link b.awk -o out.awk`.

## The `fn_` prefix convention

awkvm always emits user-defined C functions as `function fn_<name>` —
the prefix exists to avoid collision with awk built-ins (`length`,
`split`, ...). Linked helpers follow the same convention:

| Where | Name |
|---|---|
| In your `.awk` file | `function fn_clip(...)` |
| C extern declaration | `int clip(...);` |
| Inline awk call site | `__asm__("AWKVM:fn_clip(...)" ...)` |
| Awk caller (in the same script) | `fn_clip(...)` |

The asymmetry — bare name in C, `fn_`-prefixed in awk — is the same
asymmetry you already deal with when writing inline awk that calls
your own C functions. It leaks awkvm's internal naming, but only at
the same boundary where you already see `_cstr` / `_str_to_mem` /
`_alloc` / `MEM[]` and other internals.

## What linked awk can use

The runtime helpers (`_alloc` / `_load` / `_store` / `_cstr` /
`_str_to_mem` / `_memcpy` / printf machinery / ...) are available
because `--link` content goes after the runtime block. So a linked
helper can marshal C strings:

```awk
function fn_uppercase(addr,    s) {
    s = _cstr(addr)
    return _str_to_mem(toupper(s))
}
```

```c
extern char* uppercase(const char* s);
printf("%s\n", uppercase("hello"));     /* HELLO */
```

## Restrictions

- Function definitions must start at the start of a line (with at
  most leading whitespace) and use the `function fn_<name>(` form.
  awkvm's stub-suppression scanner won't see definitions written on
  unusual layouts (e.g., a leading C-style `/*` comment on the same
  line). Match awkvm's own `runtime/*.awk` formatting.

- The `fn_` prefix is required for the C ↔ awk wiring to work.
  Functions defined without it stay invisible to C externs (though
  they're still reachable from inline awk and other linked helpers
  by their bare name — useful for "private" helpers).

- No type checking: C declares `int clip(int, int, int)`, awk
  receives whatever values awkvm passes. Awk has one numeric type,
  so `int` / `long` / `double` are interchangeable at the boundary;
  pointer args arrive as integer MEM addresses (use `_cstr` etc. to
  materialize content).

- Linked content is single-namespace: function names must be unique
  across all `--link` files and across awkvm's runtime helpers
  (which already use the `_<name>` form, so collision needs you to
  pick `fn_<x>` matching `_<x>` deliberately).

## See also

- [`docs/inline-awk.md`](inline-awk.md) — inline awk via
  `__asm__("AWKVM:...")`. The two features compose: linked helpers
  written in raw awk, C glue that calls them through `extern`,
  inline awk for site-specific snippets.
- [`examples/link_basic.c`](../examples/link_basic.c) +
  [`examples/link_basic.awk`](../examples/link_basic.awk) — the
  fixture this guide walks through, exercised by `cargo test`.
