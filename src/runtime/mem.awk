function _alloc(size,    a) {
    a = NEXT_ADDR
    NEXT_ADDR += size
    return a
}
# Little-endian load: read ceil(bits/8) bytes from MEM and sign-extend.
# Undefined keys read as 0, so unwritten memory acts as zero-padded.
function _load(addr, bits,    n, i, v, sign) {
    n = int((bits + 7) / 8)
    v = 0
    for (i = 0; i < n; i++) {
        v += MEM[addr + i] * (256 ^ i)
    }
    sign = 2 ^ (bits - 1)
    return v >= sign ? v - 2 * sign : v
}
function _store(addr, val, bits,    n, i, u) {
    n = int((bits + 7) / 8)
    u = val < 0 ? val + 2 ^ bits : val
    for (i = 0; i < n; i++) {
        MEM[addr + i] = u % 256
        u = int(u / 256)
    }
}
# Read a NUL-terminated byte string from MEM into an awk string.
function _cstr(addr,    s, b) {
    s = ""
    while ((b = MEM[addr]) != 0) {
        s = s sprintf("%c", b)
        addr++
    }
    return s
}
function _memcpy(dst, src, n,    i) {
    for (i = 0; i < n; i++) MEM[dst + i] = MEM[src + i]
    return dst
}
function _memmove(dst, src, n,    i) {
    if (dst < src) {
        for (i = 0; i < n; i++) MEM[dst + i] = MEM[src + i]
    } else {
        for (i = n - 1; i >= 0; i--) MEM[dst + i] = MEM[src + i]
    }
    return dst
}
function _memset(p, v, n,    i, b) {
    b = v < 0 ? v + 256 : v
    b = b % 256
    for (i = 0; i < n; i++) MEM[p + i] = b
    return p
}
# Darwin libc: memset_pattern{4,8,16} fill `n` bytes of `dst` by repeating
# the byte pattern at `pat`. Used by clang as a memset optimization.
function _memset_pattern(dst, pat, n, plen,    i) {
    for (i = 0; i < n; i++) MEM[dst + i] = MEM[pat + (i % plen)]
}

# Darwin libc inlines isalpha / isdigit / isspace / etc. as a load from
# `_DefaultRuneLocale.__runetype[c] & FLAG` (offset 60 + c*4). The struct
# is `external` in our IR, so codegen allocates it and calls this helper
# to populate the ASCII range. The flag bits are Darwin's _CTYPE_* values
# from `<_ctype.h>`. Without this, every ctype call returns 0, which
# silently breaks anything that parses input — argparser, std::stoi,
# json scanners, etc.
function _init_ctype(base,    i, m,
                    A, C, D, G, L, P, S, U, X, B, R) {
    A = 256        # alpha     0x100
    C = 512        # control   0x200
    D = 1024       # digit     0x400
    G = 2048       # graph     0x800
    L = 4096       # lower     0x1000
    P = 8192       # punct     0x2000
    S = 16384      # space     0x4000
    U = 32768      # upper     0x8000
    X = 65536      # xdigit    0x10000
    B = 131072     # blank     0x20000
    R = 262144     # printable 0x40000
    for (i = 0; i < 256; i++) {
        m = 0
        if (i < 32 || i == 127) m += C
        if ((i >= 65 && i <= 90) || (i >= 97 && i <= 122)) m += A + G + R
        if (i >= 65 && i <= 90) m += U
        if (i >= 97 && i <= 122) m += L
        if (i >= 48 && i <= 57)  m += D + G + R + X
        if ((i >= 65 && i <= 70) || (i >= 97 && i <= 102)) m += X
        if (i == 32) m += S + B + R
        if (i == 9)  m += S + B + C
        if (i == 10 || i == 11 || i == 12 || i == 13) m += S + C
        # ASCII punctuation: printables that aren't alphanumeric or space.
        if ((i >= 33 && i <= 47) || (i >= 58 && i <= 64) || (i >= 91 && i <= 96) || (i >= 123 && i <= 126)) m += P + G + R
        _store(base + 60 + i * 4, m, 32)
    }
}
