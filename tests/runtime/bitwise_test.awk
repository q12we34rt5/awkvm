BEGIN {
    # _ashr (arithmetic shift right; floors toward -inf, like LLVM ashr)
    _assert_eq(_ashr(8, 1), 4, "_ashr(8, 1)")
    _assert_eq(_ashr(-8, 1), -4, "_ashr(-8, 1)")
    _assert_eq(_ashr(-7, 1), -4, "_ashr(-7, 1) floors")
    _assert_eq(_ashr(0, 5), 0, "_ashr(0, 5)")
    _assert_eq(_ashr(1024, 4), 64, "_ashr(1024, 4)")

    # _zext: reinterpret signed-at-width as unsigned
    _assert_eq(_zext(-1, 8), 255, "_zext(-1, 8)")
    _assert_eq(_zext(-1, 32), 4294967295, "_zext(-1, 32)")
    _assert_eq(_zext(127, 8), 127, "_zext(127, 8) unchanged")
    _assert_eq(_zext(0, 16), 0, "_zext(0, 16)")

    # _trunc: keep low w bits, sign-extend back
    _assert_eq(_trunc(255, 8), -1, "_trunc(255, 8)")
    _assert_eq(_trunc(127, 8), 127, "_trunc(127, 8)")
    _assert_eq(_trunc(128, 8), -128, "_trunc(128, 8)")
    _assert_eq(_trunc(256, 8), 0, "_trunc(256, 8) wraps")
    _assert_eq(_trunc(-1, 16), -1, "_trunc(-1, 16) preserves sign")

    # _and / _or / _xor at width 8 (signed-domain operands)
    _assert_eq(_and(240, 15, 8), 0, "_and 0xF0 & 0x0F")
    _assert_eq(_and(-1, 15, 8), 15, "_and -1 & 0x0F")
    _assert_eq(_or(240, 15, 8), -1, "_or 0xF0 | 0x0F = 0xFF (signed -1)")
    _assert_eq(_xor(255, 15, 8), -16, "_xor 0xFF ^ 0x0F = 0xF0 (signed -16)")
    _assert_eq(_and(-1, -1, 32), -1, "_and -1 & -1 width 32")

    # _shl at width 8: high bit becomes sign, overflow wraps
    _assert_eq(_shl(1, 4, 8), 16, "_shl 1<<4")
    _assert_eq(_shl(1, 7, 8), -128, "_shl 1<<7 = 0x80 (signed -128)")
    _assert_eq(_shl(1, 8, 8), 0, "_shl 1<<8 wraps to 0")
    _assert_eq(_shl(3, 6, 8), -64, "_shl 3<<6 = 0xC0 (signed -64)")

    # _lshr at width 8: shift in zeros even for negative inputs
    _assert_eq(_lshr(-16, 4, 8), 15, "_lshr 0xF0>>4")
    _assert_eq(_lshr(-1, 4, 8), 15, "_lshr 0xFF>>4")
    _assert_eq(_lshr(-128, 1, 8), 64, "_lshr 0x80>>1 = 0x40")
}
