# Inline awk

Drop into raw awk from C / C++ via `__asm__("AWKVM:...")`. clang lowers
inline assembly to a special `call asm` in the IR; awkvm recognizes the
`AWKVM:` prefix and emits the body verbatim into the surrounding awk
function, with `%N` operand placeholders substituted from the C-side
variables.

## Basic syntax

GNU extended asm format applies — the C side declares output operands,
input operands, and clobbers; the asm template references them as
`%0`, `%1`, … in declaration order (outputs first, then inputs):

```c
__asm__("AWKVM:awk-body"
        : output_constraints
        : input_constraints
        : clobbers);
```

Use `"r"` as the constraint for both inputs and outputs (read it as
"register" — meaningless to awkvm, but clang requires _some_ constraint).
Output constraints are prefixed with `=`. Suppress clang's
`-Wasm-operand-widths` warning at the top of the file:

```c
#pragma clang diagnostic ignored "-Wasm-operand-widths"
```

A trivial example:

```c
int x = 7;
int sq;
__asm__("AWKVM:%0 = %1 * %1" : "=r"(sq) : "r"(x));
// awkvm emits:  r_sq = r_x * r_x
// (or constant-folded if -O1 sees x as known)
```

## Passing values

### C → awk: input operands

Primitive values cross the boundary as awk numbers directly:

```c
int n = 10;
double d = 3.14;
int total;
__asm__("AWKVM:%0 = int(%1 * %2)" : "=r"(total) : "r"(n), "r"(d));
// total = 31
```

### awk → C: output operands

Numeric outputs are written back into the C variable:

```c
long ms;
__asm__("AWKVM:%0 = systime() * 1000" : "=r"(ms));
```

### C strings ⇄ awk strings

`char*` in awkvm is a byte address into `MEM[]`; awk strings are
proper awk values. Two runtime helpers bridge the gap:

| Direction | Helper | Where it's defined |
|---|---|---|
| `MEM[addr…]` (NUL-terminated) → awk string | `_cstr(addr)` | `runtime/mem.awk` |
| awk string → fresh `MEM` allocation, returns address | `_str_to_mem(s)` | `runtime/str.awk` |

```c
const char* in = "Hello, World";
char* out;
__asm__(
    "AWKVM:s = _cstr(%1); "
    "s = toupper(s); "
    "%0 = _str_to_mem(s)"
    : "=r"(out)
    : "r"(in)
);
printf("%s\n", out);   // HELLO, WORLD
```

The `_str_to_mem` allocation is owned by awkvm's bump allocator — not
freed (in keeping with the rest of awkvm's memory model). Don't try
to `free()` what comes back.

## Recipes

### Numeric compute, multi-input

```c
int a = 3, b = 4, c = 5;
int r;
__asm__("AWKVM:%0 = %1 * %2 + %3" : "=r"(r) : "r"(a), "r"(b), "r"(c));
// r = 17
```

### Subprocess capture (single line)

```c
char* greeting;
__asm__(
    "AWKVM:cmd = \"printf hello\"; "
    "cmd | getline line; "
    "close(cmd); "
    "%0 = _str_to_mem(line)"
    : "=r"(greeting)
);
// greeting → "hello"
```

### Subprocess capture (multi-line, accumulate)

```c
char* listing;
__asm__(
    "AWKVM:cmd = \"ls -1 /tmp\"; "
    "out = \"\"; "
    "while ((cmd | getline line) > 0) "
    "    out = out line \"\\n\"; "
    "close(cmd); "
    "%0 = _str_to_mem(out)"
    : "=r"(listing)
);
```

### Subprocess send (write to stdin)

```c
const char* msg = "hello world";
int letter_count;
__asm__(
    "AWKVM:cmd = \"wc -c\"; "
    "print _cstr(%1) | cmd; "
    "close(cmd, \"to\"); "        // half-close: signal EOF on cmd's stdin
    "cmd | getline %0; "          // now read its stdout
    "close(cmd)"
    : "=r"(letter_count)
    : "r"(msg)
);
```

### Bidirectional coprocess (gawk extension)

```c
char* sorted;
__asm__(
    "AWKVM:cmd = \"sort\"; "
    "print \"banana\" |& cmd; "
    "print \"apple\"  |& cmd; "
    "print \"cherry\" |& cmd; "
    "close(cmd, \"to\"); "
    "out = \"\"; "
    "while ((cmd |& getline ln) > 0) out = out ln \"\\n\"; "
    "close(cmd); "
    "%0 = _str_to_mem(out)"
    : "=r"(sorted)
);
// sorted → "apple\nbanana\ncherry\n"
```

### File I/O

Read one line:

```c
const char* path = "/etc/hostname";
char* hostname;
__asm__(
    "AWKVM:fn = _cstr(%1); "
    "getline ln < fn; "
    "close(fn); "
    "%0 = _str_to_mem(ln)"
    : "=r"(hostname)
    : "r"(path)
);
```

Read entire file:

```c
__asm__(
    "AWKVM:fn = _cstr(%1); "
    "out = \"\"; "
    "while ((getline ln < fn) > 0) out = out ln \"\\n\"; "
    "close(fn); "
    "%0 = _str_to_mem(out)"
    : "=r"(buf)
    : "r"(path)
);
```

Write a file:

```c
const char* path = "/tmp/out.txt";
const char* msg  = "hello\n";
__asm__(
    "AWKVM:fn = _cstr(%0); "
    "printf \"%s\", _cstr(%1) > fn; "
    "close(fn)"
    : : "r"(path), "r"(msg)
);
```

### Regex (gsub / sub / match)

```c
const char* in = "hello world";
char* out;
__asm__(
    "AWKVM:s = _cstr(%1); "
    "gsub(/o/, \"0\", s); "
    "%0 = _str_to_mem(s)"
    : "=r"(out)
    : "r"(in)
);
// out → "hell0 w0rld"
```

`match`, `sub`, `split` etc. all available the same way.

### Time and environment

```c
char* now;
__asm__(
    "AWKVM:%0 = _str_to_mem(strftime(\"%Y-%m-%d %H:%M:%S\"))"
    : "=r"(now)
);

const char* var = "PATH";
char* val;
__asm__(
    "AWKVM:%0 = _str_to_mem(ENVIRON[_cstr(%1)])"
    : "=r"(val)
    : "r"(var)
);
```

### Associative arrays / histograms

```c
const char* text = "the quick brown fox jumps over the lazy dog the lazy dog";
char* histogram;
__asm__(
    "AWKVM:n = split(_cstr(%1), words, \" \"); "
    "for (i = 1; i <= n; i++) freq[words[i]]++; "
    "out = \"\"; "
    "for (w in freq) out = out w \"=\" freq[w] \"\\n\"; "
    "%0 = _str_to_mem(out)"
    : "=r"(histogram)
    : "r"(text)
);
```

### `system()` (one-shot, no capture)

```c
const char* cmd = "ls -l /tmp";
__asm__("AWKVM:system(_cstr(%0))" : : "r"(cmd));
```

## Escape rules

awkvm unescapes both forms LLVM uses inside asm template strings:

- `\\` → `\`  (literal backslash)
- `\HH` → byte 0xHH (e.g. `\22` → `"`, `\0A` → newline)

So inside the C asm string you can use the usual escape sequences:

```c
"AWKVM:print \"hello\""        // → print "hello"
"AWKVM:s = \"a\\tb\""           // → s = "a\tb"  (awk reads as "a<TAB>b")
"AWKVM:s = \"line1\\nline2\""   // → s = "line1\nline2"
```

A literal `$` in the awk source needs `$$` in the C asm string (to
keep clang from interpreting it as an operand placeholder):

```c
"AWKVM:print $$9"              // → print $9   (awk's 9th field)
```

## Multi-line bodies

Two ways:

```c
// Option 1: ; separator (awk allows multiple statements per line).
__asm__("AWKVM:a = 1; b = 2; print a + b");

// Option 2: real \n in the asm string. awkvm splits on newline and
// re-indents each line.
__asm__(
    "AWKVM:a = 1\n"
    "b = 2\n"
    "print a + b"
);
```

The C-string concatenation in option 2 is sugar; both produce the same
awk output.

## Limitations

- **Single output operand max.** Multiple outputs lower to a struct
  return + `extractvalue`, which awkvm bails on. Use multiple inputs
  to a single output; or call back via memory if you really need
  several values.
- **No top-level structure.** Inline awk runs inside an awkvm-emitted
  function body, so you can't write `BEGIN { ... }`, `END { ... }`,
  pattern-action rules (`/regex/ { ... }`), or new top-level awk
  functions inside the asm string. The TODO items for whole-function
  awk bodies (`__attribute__((annotate("awkvm_body:...")))`) and
  `awkvm --link helpers.awk` cover those use cases.
- **String marshaling is manual.** No automatic `const char*` →
  awk-string at the call boundary; you write `_cstr(%N)` /
  `_str_to_mem(s)` explicitly. A type-inferring wrapper layer is
  possible future work.
- **No type checking on operands.** `"r"` constraint is opaque;
  passing a struct or non-numeric type is silently wrong.
- **Toolchain pin still applies.** Inline awk doesn't escape the
  general "awkvm needs the same clang as built `PROBE_MAP`" rule
  (see `LIMITATIONS.md` "Toolchain coupling").

## Reference fixtures

| Fixture | Pattern shown |
|---|---|
| [`examples/inline_awk.c`](../examples/inline_awk.c) | `%N` substitution with multiple inputs / single output |
| [`examples/inline_awk_str.c`](../examples/inline_awk_str.c) | `_cstr` / `_str_to_mem` marshal via `toupper` |
| [`examples/inline_awk_pipe.c`](../examples/inline_awk_pipe.c) | `cmd \| getline` subprocess capture |
| [`examples/inline_awk_regex.c`](../examples/inline_awk_regex.c) | `gsub` / regex from C |

Each is a `cargo test` regression so the patterns above stay
lockstep with what the implementation produces.
