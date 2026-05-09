# Tiny C / C++ ABI bridge. Each helper here is a thin wrapper around a
# core runtime function under the canonical libc / Itanium-ABI name, so
# codegen can emit `fn_<name>(...)` uniformly without special-casing.
#
# Helpers are emitted block-by-block by codegen and skipped when the
# user's own program defines a function of the same C name (so a custom
# operator new, or an override of malloc / free, takes precedence over
# the default wrapper here).
#
# Block boundary convention: each helper is one function preceded by an
# optional `# ...` comment block, separated from the next helper by a
# single blank line. The codegen-side splitter relies on this layout.

function fn_puts(addr) {
    print _cstr(addr)
    return 0
}

function fn_putchar(c) {
    printf "%c", c
    return c
}

function fn_malloc(size) {
    return _alloc(size)
}

# Bump allocator never reclaims; free is a no-op.
function fn_free(p) {}

function fn_exit(code) {
    exit code
}

function fn_abort() {
    exit 134
}

function fn_memcpy(dst, src, n) {
    return _memcpy(dst, src, n)
}

function fn_memmove(dst, src, n) {
    return _memmove(dst, src, n)
}

function fn_memset(p, v, n) {
    return _memset(p, v, n)
}

# Darwin _FORTIFY_SOURCE wraps memcpy / memmove / memset in `__X_chk`,
# adding a 4th `dst_size` arg used for runtime overflow checking.
# The check itself is a hardening feature; for awkvm we just discard
# it and forward to the unchecked helper.
function fn___memcpy_chk(dst, src, n, dst_size)  { return _memcpy(dst, src, n) }
function fn___memmove_chk(dst, src, n, dst_size) { return _memmove(dst, src, n) }
function fn___memset_chk(p, v, n, dst_size)      { return _memset(p, v, n) }

# Darwin libc fixed-stride pattern fills.
function fn_memset_pattern4(dst, pat, n) {
    _memset_pattern(dst, pat, n, 4)
}

function fn_memset_pattern8(dst, pat, n) {
    _memset_pattern(dst, pat, n, 8)
}

function fn_memset_pattern16(dst, pat, n) {
    _memset_pattern(dst, pat, n, 16)
}

function fn_strlen(p) {
    return _strlen(p)
}

function fn_strcmp(a, b) {
    return _strcmp(a, b)
}

# Integer / float string parsing. base / endptr arguments are accepted
# for ABI compatibility but ignored — the dominant call shape is
# strtol(s, NULL, 10) and strtod(s, NULL).
function fn_atoi(s) { return _atoi(s) }
function fn_atol(s) { return _atoi(s) }
function fn_atoll(s) { return _atoi(s) }
function fn_strtol(s, end, base) { return _atoi(s) }
function fn_strtoll(s, end, base) { return _atoi(s) }
function fn_strtoul(s, end, base) { return _atoi(s) }
function fn_strtoull(s, end, base) { return _atoi(s) }

function fn_atof(s) { return _atof(s) }
function fn_strtod(s, end) { return _atof(s) }
function fn_strtof(s, end) { return _atof(s) }
function fn_strtold(s, end) { return _atof(s) }

# system() — gawk system(s) is already blocking fork+exec.
function fn_system(cmd) { return system(_cstr(cmd)) }

# Math libc bridges. Most of <cmath> lowers to LLVM intrinsics at -O1
# and is handled in src/codegen/call.rs; the few that don't (atan2 and
# hypot, plus the libm fallbacks at -O0) need direct fn_<name> entries
# so codegen can emit a regular call.
function fn_atan2(y, x) { return atan2(y, x) }
function fn_hypot(x, y) { return sqrt(x * x + y * y) }

# stdio FILE* bridge. fopen allocates a 1-byte address as the FILE*
# handle, registers it in the stream tables, and returns the address.
# NULL (0) is returned when the mode string isn't recognized; gawk's
# open-on-first-write means a bad path doesn't surface until later
# writes/reads fail. Modes: "r" / "w" / "a" with optional "b" suffix
# (binary; ignored — gawk has no text/binary distinction). "+" forms
# (read+write) aren't supported and return NULL.
function fn_fopen(path, mode,    addr, p, m) {
    p = _cstr(path)
    m = _cstr(mode)
    addr = _alloc(1)
    if (m == "w" || m == "wb")      _stream_open_w(addr, p, "file_w")
    else if (m == "a" || m == "ab") _stream_open_w(addr, p, "file_a")
    else if (m == "r" || m == "rb") _stream_open_r(addr, p, "file_r")
    else                            return 0
    return addr
}

function fn_fclose(fp) {
    _stream_close(fp)
    return 0
}

# fwrite / fread always succeed in our model (gawk doesn't surface
# I/O errors at the byte level). fread returns the floor of bytes-
# read / element-size on EOF mid-element, matching the C contract.
# Note: line-buffered read fabricates a trailing "\n" when the
# source's final line has no newline — fine for text, lossy for
# binary files that don't happen to end on a newline boundary.
function fn_fwrite(buf, size, count, fp,    total, i) {
    total = size * count
    for (i = 0; i < total; i++) _stream_write_byte(fp, MEM[buf + i])
    return count
}

function fn_fread(buf, size, count, fp,    total, i, b) {
    total = size * count
    for (i = 0; i < total; i++) {
        b = _stream_read_byte(fp)
        if (b < 0) return int(i / size)
        MEM[buf + i] = b
    }
    return count
}

function fn_fputc(c, fp) {
    _stream_write_byte(fp, c)
    return c
}

# fputs writes the C-string at addr (NUL-terminated, no trailing newline)
# to fp. Returns 0 on success per ISO C (non-negative).
function fn_fputs(addr, fp,    i, b) {
    i = 0
    while ((b = MEM[addr + i]) != 0) {
        _stream_write_byte(fp, b)
        i++
    }
    return 0
}

function fn_fgetc(fp) { return _stream_read_byte(fp) }

# fgets reads up to size-1 bytes (or until newline / EOF), NUL-
# terminates, returns the buffer address (0 / NULL on immediate EOF).
function fn_fgets(buf, size, fp,    i, b) {
    if (size <= 1) return 0
    i = 0
    while (i < size - 1) {
        b = _stream_read_byte(fp)
        if (b < 0) {
            if (i == 0) return 0
            break
        }
        MEM[buf + i] = b
        i++
        if (b == 10) break    # newline ends the read but is included
    }
    MEM[buf + i] = 0
    return buf
}

# popen / pclose — pipe streams. "r" reads a child's stdout via
# `cmd | getline`; "w" feeds a child's stdin via `print | cmd`.
# pclose surfaces the child exit status from gawk's close().
function fn_popen(cmd, mode,    addr, c, m) {
    c = _cstr(cmd)
    m = _cstr(mode)
    addr = _alloc(1)
    if (m == "r")      _stream_open_r(addr, c, "pipe_r")
    else if (m == "w") _stream_open_w(addr, c, "pipe_w")
    else               return 0
    return addr
}

function fn_pclose(fp) { return _stream_close(fp) }

# C++ exception ABI. __cxa_throw sets the global unwind state; the
# caller's post-call `if (UNWINDING) return` then propagates the throw
# up the stack until a landingpad clears it.
function fn___cxa_allocate_exception(size) {
    return _alloc(size)
}

function fn___cxa_throw(obj, typeinfo, dtor) {
    EXC_OBJ = obj
    EXC_TYPE_ID = typeinfo
    UNWINDING = 1
}

function fn___cxa_begin_catch(p) {
    return p
}

function fn___cxa_end_catch() {}

function fn___cxa_rethrow() {
    UNWINDING = 1
}

# Static-local one-time init. The Itanium ABI uses an i64 guard byte:
# bit 0 == 1 means "already initialized". `acquire` returns nonzero if
# THIS caller should run the initializer (and leaves the guard in an
# in-progress state); `release` finalizes it. We're single-threaded so
# the ABI's atomicity / threading concerns don't apply — just track
# initialized-or-not via MEM[guard] and short-circuit on subsequent
# calls. Without this, the default 0-return makes the runtime skip
# every static-local initializer (e.g. `static const X = ctorExpr()`),
# which silently leaves zeroinit data in place.
function fn___cxa_guard_acquire(guard) {
    if (_load(guard, 8) != 0) return 0
    return 1
}
function fn___cxa_guard_release(guard) {
    _store(guard, 1, 8)
}
function fn___cxa_guard_abort(guard) {
    _store(guard, 0, 8)
}

# Itanium-mangled operator new / delete (and the nothrow / sized variants).
function fn__Znwm(size) { return _alloc(size) }
function fn__Znam(size) { return _alloc(size) }
function fn__ZnwmRKSt9nothrow_t(size, t) { return _alloc(size) }
function fn__ZnamRKSt9nothrow_t(size, t) { return _alloc(size) }
function fn__ZdlPv(p) {}
function fn__ZdaPv(p) {}
function fn__ZdlPvm(p, n) {}
function fn__ZdaPvm(p, n) {}
function fn__ZdlPvRKSt9nothrow_t(p, t) {}
function fn__ZdaPvRKSt9nothrow_t(p, t) {}
