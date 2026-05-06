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
