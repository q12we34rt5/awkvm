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
function _udiv(a, b, w,    ua, ub, r) {
    ua = a < 0 ? a + 2 ^ w : a
    ub = b < 0 ? b + 2 ^ w : b
    r = int(ua / ub)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}

function _urem(a, b, w,    ua, ub, r) {
    ua = a < 0 ? a + 2 ^ w : a
    ub = b < 0 ? b + 2 ^ w : b
    r = ua - int(ua / ub) * ub
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

# popcount / count-leading-zeros / count-trailing-zeros / bswap.
# Used by libc++ hash table internals (ctpop on hash codes).
# Operands come in as awkvm's signed-model integers; zext to
# unsigned before bit-walking.
function _popcount(a,    ua, n) {
    ua = a < 0 ? a + 2 ^ 64 : a
    n = 0
    while (ua > 0) {
        n += ua % 2
        ua = int(ua / 2)
    }
    return n
}

function _ctlz(a, w,    ua, n) {
    ua = a < 0 ? a + 2 ^ w : a
    if (ua == 0) return w
    n = 0
    while (ua < 2 ^ (w - 1)) { ua *= 2; n++ }
    return n
}

function _cttz(a, w,    ua, n) {
    ua = a < 0 ? a + 2 ^ w : a
    if (ua == 0) return w
    n = 0
    while (ua % 2 == 0) { ua = int(ua / 2); n++ }
    return n
}

# fshl / fshr — funnel shifts. Concatenate (a, b) as a 2w-bit
# value with a in the high half, shift, take the matching w bits.
# fshl(a, b, n) = high w bits of ((a:b) << n).
# fshr(a, b, n) = low  w bits of ((a:b) >> n).
# Hash mixing in libc++ uses these as fixed-width rotates.
function _fshl(a, b, n, w,    ua, ub, k, r) {
    ua = a < 0 ? a + 2 ^ w : a
    ub = b < 0 ? b + 2 ^ w : b
    k = n % w
    if (k == 0) r = ua
    else        r = (ua * 2 ^ k) % (2 ^ w) + int(ub / 2 ^ (w - k))
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}

function _fshr(a, b, n, w,    ua, ub, k, r) {
    ua = a < 0 ? a + 2 ^ w : a
    ub = b < 0 ? b + 2 ^ w : b
    k = n % w
    if (k == 0) r = ub
    else        r = (ua * 2 ^ (w - k)) % (2 ^ w) + int(ub / 2 ^ k)
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}

function _bswap(a, w,    ua, n, i, b, r) {
    ua = a < 0 ? a + 2 ^ w : a
    n = w / 8
    r = 0
    for (i = 0; i < n; i++) {
        b = int(ua / (256 ^ i)) % 256
        r += b * (256 ^ (n - 1 - i))
    }
    return r >= 2 ^ (w - 1) ? r - 2 ^ w : r
}
