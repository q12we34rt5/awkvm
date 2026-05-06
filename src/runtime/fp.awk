# IEEE 754 single-precision pack/unpack. Subnormals collapse to 0 and
# inf / NaN are not preserved (Phase 8 limitation).
function _f32_to_bits(v,    sign, av, e, m) {
    if (v == 0) return 0
    if (v < 0) { sign = 1; av = -v } else { sign = 0; av = v }
    e = 0
    while (av >= 2) { av /= 2; e++ }
    while (av < 1) { av *= 2; e-- }
    m = int((av - 1) * 8388608)
    if (e + 127 <= 0) return sign * 2147483648
    if (e + 127 >= 255) return sign * 2147483648 + 2139095040
    return sign * 2147483648 + (e + 127) * 8388608 + m
}
function _f32_from_bits(raw,    sign, bexp, mant) {
    if (raw < 0) raw += 4294967296
    sign = int(raw / 2147483648) % 2
    bexp = int(raw / 8388608) % 256
    mant = raw % 8388608
    if (bexp == 0)   return 0
    if (bexp == 255) return 0
    return (sign ? -1 : 1) * (1 + mant / 8388608) * (2 ^ (bexp - 127))
}
function _load_f32(addr) { return _f32_from_bits(_load(addr, 32)) }
function _store_f32(addr, v) { _store(addr, _f32_to_bits(v), 32) }
# IEEE 754 double-precision via two halves; the 64-bit raw pattern
# wouldn't fit in awk's 53-bit-safe integer range so we never assemble it.
function _load_f64(addr,    lo, hi, sign, bexp, mhi, m) {
    lo = _load(addr, 32);     if (lo < 0) lo += 4294967296
    hi = _load(addr + 4, 32); if (hi < 0) hi += 4294967296
    sign = int(hi / 2147483648) % 2
    bexp = int(hi / 1048576) % 2048
    mhi  = hi % 1048576
    m = mhi * 4294967296 + lo
    if (bexp == 0)    return 0
    if (bexp == 2047) return 0
    return (sign ? -1 : 1) * (1 + m / 4503599627370496) * (2 ^ (bexp - 1023))
}
function _store_f64(addr, v,    sign, av, e, m, mhi, mlo) {
    if (v == 0) { _store(addr, 0, 32); _store(addr + 4, 0, 32); return }
    if (v < 0) { sign = 1; av = -v } else { sign = 0; av = v }
    e = 0
    while (av >= 2) { av /= 2; e++ }
    while (av < 1) { av *= 2; e-- }
    m = (av - 1) * 4503599627370496
    mhi = int(m / 4294967296)
    mlo = int(m - mhi * 4294967296)
    if (e + 1023 <= 0) {
        _store(addr, 0, 32); _store(addr + 4, sign * 2147483648, 32); return
    }
    if (e + 1023 >= 2047) {
        _store(addr, 0, 32); _store(addr + 4, sign * 2147483648 + 2146435072, 32); return
    }
    _store(addr, mlo, 32)
    _store(addr + 4, sign * 2147483648 + (e + 1023) * 1048576 + mhi, 32)
}
