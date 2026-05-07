# File and stream I/O

awkvm v0.3.0 unifies libc `FILE*` and C++ `<fstream>` on a single
address-keyed stream subsystem. The same `_STREAM_*` runtime
tables back both API surfaces, so a program that mixes
`fwrite(fp, ...)` and `ofstream f; f << ...` reads identically
back through either side.

## Quick example

```c
// libc-side: fopen + fwrite + fread + fclose
FILE* fp = fopen(path, "w");
fwrite("from libc\n", 1, 10, fp);
fclose(fp);
```

```cpp
// C++-side: ofstream<< + ifstream::read + gcount
std::ofstream f(path);
f.write("from ofstream\n", 14);
f.close();

char buf[64];
std::ifstream g(path);
g.read(buf, 64);
long n = g.gcount();
```

Run with `LC_ALL=C gawk -f program.awk` (the locale check fires
loudly otherwise — see "Locale" below).

## Stream model

Six address-keyed tables. A "stream address" is whatever the user's
language hands us — `FILE*` from `fopen` (a 1-byte placeholder
returned from `_alloc`), or the `this` of an `ofstream` / `ifstream`
instance (and its filebuf at +8).

| Table | Holds |
| --- | --- |
| `_STREAM_DEST[addr]` | gawk redirect target for writes (file path, pipe command, "/dev/stderr"). Empty / absent → stdout. |
| `_STREAM_SRC[addr]` | gawk source for reads (file path or pipe command). |
| `_STREAM_KIND[addr]` | routing tag — `file_w`, `file_a`, `file_r`, `pipe_w`, `pipe_r`. Picks the gawk redirect operator (`>`, `>>`, `<`, `\|`, `cmd \| getline`). |
| `_STREAM_BUF[addr]` | line-buffer for byte-level reads. |
| `_STREAM_POS[addr]` | 1-indexed cursor into `_STREAM_BUF`. |
| `_STREAM_EOF[addr]` | sticky 1 once the source returns EOF. |

Both `<fstream>` constructor probes and `fopen` / `popen` register
into these tables; both consumers (`fwrite`, `<<`, `read`, `>>`,
etc.) look up by address. No two-level indirection — see
"`rdbuf` swap" below for the consequences.

## libc surface

| Function | Notes |
| --- | --- |
| `fopen(path, mode)` | Modes `"r"` / `"w"` / `"a"` (and `"rb"` / `"wb"` / `"ab"`). Returns a 1-byte address. NULL (0) for unrecognized modes. `"+"` forms not supported. |
| `fwrite` / `fread` | Byte loop over `MEM[buf..]`. `fread` returns floor(bytes_read / size) on partial-element EOF, matching the C contract. |
| `fclose` | Closes the gawk-side handle; drops all per-stream tables. |
| `fputc` / `fputs` | Single byte / NUL-terminated C-string write. |
| `fgetc` / `fgets` | Single byte / line-terminated read. `fgets` includes the newline if hit before size-1. |
| `fprintf` / `printf` | Same `_format` engine; `fprintf` routes the formatted string through `_stream_write_str(stream, ...)` instead of bare gawk `printf`. Specs: `%d %i %u %x %X %o %c %s %p %f %F %g %G %e %E %%`. |
| `scanf` / `fscanf` / `sscanf` | `scanf` reads from `/dev/stdin` via a lazily-registered sentinel stream; `fscanf` works on any FILE\* registered by `fopen`; `sscanf` preloads `_STREAM_BUF` from a NUL-terminated cstring in MEM (no SRC, so the engine consumes the buffer once and stops). Specs: `%d %i %ld %u %x %lo %f %lf %g %le %s %c`. Width + `*` (assignment-suppression) ignored. Returns count of items assigned. |
| `popen(cmd, mode)` / `pclose` | `mode == "r"` reads child stdout via `cmd \| getline`; `"w"` feeds child stdin via `print \| cmd`. `pclose` returns gawk's `close()` status (child exit code). |
| `system(cmd)` | Direct wrap of gawk's blocking `system()`. |

## C++ `<fstream>` surface

| C++ form | Routing |
| --- | --- |
| `std::ofstream f(path)` | Constructor probe → registers `_STREAM_DEST[f]` and `_STREAM_DEST[f+8]` (the filebuf at +8). |
| `std::ifstream f(path)` | Same shape, registers `_STREAM_SRC[f]` / `_STREAM_SRC[f+8]`. |
| `f << x` | Inherited from ostream — uses the existing `_ostream_*` probes (int / long / unsigned / double / cstr / bool / void\*). |
| `f >> x` | Inherited from istream — `_istream_*` probes for int / long / unsigned / double. |
| `f.write(buf, n)` / `f.put(c)` | Block + single-byte write. `write` shares the byte-loop helper with `<< "literal"`. |
| `f.read(buf, n)` / `f.gcount()` / `f.get()` | Block + single-byte read. `gcount()` is **not** probed — clang -O1 inlines it to `_load(this+8, 64)`, so `_istream_read` / `_istream_get` `_store` the count at `MEM[stream+8]` before returning, and the inlined gcount load picks it up. |
| `f.close()` | Routed through `basic_filebuf::close` (the mangled name `f.close()` actually resolves to). Closes the gawk-side handle, drops the stream tables. Without this probe, gawk's redirected output stays buffered and read-after-write to the same file in one program reads stale state. |

## Cross-API demo

[`examples/io/io_mixed.cpp`](../examples/io/io_mixed.cpp) writes
two files — one via `fopen + fwrite + fclose`, one via
`ofstream + << + close()` — then reads both back via `fopen +
fread`. The fact that the C-side reader sees the C++-side writer's
content (and vice versa) is the integration test the unified
`_STREAM_*` tables were built for.

## Locale

awkvm's stream subsystem assumes single-byte string semantics
(`length(c)` of any byte returns 1). gawk delivers that under
`LC_ALL=C`; multi-byte locales (UTF-8, default on macOS / most
Linux) make `length("→") == 1`, breaking byte-level fread / fwrite
/ `_cstr` / inline-awk-byte-ops silently. v0.3.0 hard-fails at
startup if the locale isn't C-compatible:

```
awkvm: gawk is in a multi-byte locale (length("中") = 1 ≠ 3).
Byte-level I/O depends on single-byte string semantics — rerun
with `LC_ALL=C gawk -f ...`.
```

The check fires for every program (not just I/O ones), since
non-ASCII string output through `printf` / `cout << "literal"`
hits the same byte-vs-char issue.

## `rdbuf` swap not tracked

The single-level address-keyed model means redirect idioms that
swap a stream's underlying streambuf don't work in v0.3.0:

```cpp
std::ofstream log("debug.log");
std::cout.rdbuf(log.rdbuf());  // redirect cout to log
std::cout << "hello\n";        // ❌ writes to stdout, not log
```

Real C++ behavior: `cout`'s rdbuf points to log's filebuf;
`<<` walks `cout.rdbuf()->sputn(...)` which writes to debug.log.
awkvm: our `_ostream_cstr` probe template at the `<<` call site
keys on `cout`'s `this` directly — `_STREAM_DEST[cout]` is
absent, so output falls through to bare `printf` (stdout).

What this **doesn't** affect: direct fstream usage. `f << x`,
`f >> x`, `f.read(...)`, `f.write(...)` all key on `f`'s own
address, which IS registered by the constructor probe — no
indirection through rdbuf needed. fstream-only programs work.

The two-level dispatch (`ostream → streambuf id → target`) that
fixes this is grouped with `<sstream>` / `<fstream>` machinery
in v0.4.0 "iostream completion".

## Other limitations (v0.4.0 scope)

- **`std::endl` proper.** Inlines to a virtual `widen('\n')`
  through ctype facet's vtable, which awkvm doesn't model. Use
  `"\n"` literal — same observable output.
- **`std::getline(is, str)` / `cin >> std::string`.** libc++
  string is SSO-laid-out in `MEM[]` and writing requires
  layout-aware code. Deferred.
- **`<iomanip>`** (setw / setfill / setprecision / hex / oct /
  dec / fixed / scientific). Per-stream format state; needs a
  manipulator dispatch layer.
- **`<sstream>`.** In-memory streams require a streambuf
  indirection unit (same machinery as rdbuf swap).
- **State queries** (`is.eof()`, `is.fail()`, `is.good()`, etc.)
  fall through to the linkonce_odr libc++ bodies, which no-op in
  awkvm. Practical effect: they always return success-ish
  defaults. Use `gcount()` < requested as the EOF signal.
- **Open mode flags ignored.** `ofstream f(path, std::ios::app)`
  always truncates in v0.3.0 (the openmode arg isn't consulted in
  the constructor probe). `"a"` mode via `fopen` does honor
  append. Workaround: use `fopen` for non-default modes.
- **Binary fread fidelity.** Line-buffered read fabricates a
  trailing `\n` when the source's last line has no newline — fine
  for text, lossy for binary that doesn't end on a line boundary.
- **Destructors don't call close.** The translated linkonce_odr
  destructor body manipulates vtable / locale state that no-ops
  in awkvm. Call `f.close()` explicitly for mid-program
  flush-and-close; otherwise gawk's process-exit auto-flush
  handles it.

## See also

- [`examples/io/file_io.c`](../examples/io/file_io.c) — libc
  fopen / fwrite / fread / fclose round-trip.
- [`examples/io/io_mixed.cpp`](../examples/io/io_mixed.cpp) —
  cross-API demo (libc writes one file, ofstream writes another,
  both read back via libc fread).
- [`examples/io/ifstream_extract.cpp`](../examples/io/ifstream_extract.cpp)
  — ifstream + `>>` extraction round-trip for primitives.
- [`examples/io/block_io.cpp`](../examples/io/block_io.cpp) —
  ofstream::write + put → ifstream::read + gcount + get(EOF).
- [`examples/io/fprintf_basic.c`](../examples/io/fprintf_basic.c) —
  `fprintf` to a libc FILE\* with `%d / %s / %.3f`.
- [`examples/io/scanf_basic.c`](../examples/io/scanf_basic.c) —
  `scanf` reading three primitives from stdin.
- [`examples/io/fscanf_basic.c`](../examples/io/fscanf_basic.c) —
  `fprintf` write + `fscanf` read round-trip via the same FILE\*.
- [`examples/io/sscanf_basic.c`](../examples/io/sscanf_basic.c) —
  `sscanf` parsing four primitives out of an in-memory cstring.
- [`docs/inline-awk.md`](inline-awk.md) — for I/O patterns awkvm
  doesn't bind directly (gawk's coprocess `\|&`, `getline` from
  arbitrary sources, `system()` with output capture, etc.).
