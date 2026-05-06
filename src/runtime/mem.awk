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
