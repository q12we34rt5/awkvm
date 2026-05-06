function _ashr(a, n) {
    return (a < 0 && a % (2 ^ n) != 0) ? int(a / (2 ^ n)) - 1 : int(a / (2 ^ n))
}
function _zext(a, w) {
    return a < 0 ? a + (2 ^ w) : a
}
function _trunc(a, w,    m, mod) {
    mod = 2 ^ w
    m = a % mod
    if (m < 0) m += mod
    return m >= mod / 2 ? m - mod : m
}
# Bitwise wrappers that accept signed operands. gawk's and/or/xor reject
# negative inputs, so zext both sides to unsigned, run the op, then
# reinterpret the result back at width w.
function _and(a, b, w,    ua, ub, r) {
    ua = a < 0 ? a + 2 ^ w : a
    ub = b < 0 ? b + 2 ^ w : b
    r = and(ua, ub)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
function _or(a, b, w,    ua, ub, r) {
    ua = a < 0 ? a + 2 ^ w : a
    ub = b < 0 ? b + 2 ^ w : b
    r = or(ua, ub)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
function _xor(a, b, w,    ua, ub, r) {
    ua = a < 0 ? a + 2 ^ w : a
    ub = b < 0 ? b + 2 ^ w : b
    r = xor(ua, ub)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
function _shl(a, n, w,    ua, r) {
    ua = a < 0 ? a + 2 ^ w : a
    r = lshift(ua, n) % (2 ^ w)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
function _lshr(a, n, w,    ua, r) {
    ua = a < 0 ? a + 2 ^ w : a
    r = rshift(ua, n)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
