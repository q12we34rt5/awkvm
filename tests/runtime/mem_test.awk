BEGIN {
    # _alloc bumps NEXT_ADDR
    _MT_a = _alloc(8)
    _MT_b = _alloc(4)
    _assert_eq(_MT_b, _MT_a + 8, "_alloc bumps NEXT_ADDR by size")

    # _store/_load round-trip at every native width
    _MT_p = _alloc(8)
    _store(_MT_p, 42, 8)
    _assert_eq(_load(_MT_p, 8), 42, "i8 round-trip positive")
    _store(_MT_p, -42, 8)
    _assert_eq(_load(_MT_p, 8), -42, "i8 round-trip negative")
    _store(_MT_p, 0x7FFF, 16)
    _assert_eq(_load(_MT_p, 16), 0x7FFF, "i16 max positive")
    _store(_MT_p, -0x8000, 16)
    _assert_eq(_load(_MT_p, 16), -0x8000, "i16 min")
    _store(_MT_p, 0x12345678, 32)
    _assert_eq(_load(_MT_p, 32), 0x12345678, "i32 round-trip")
    _store(_MT_p, -1, 32)
    _assert_eq(_load(_MT_p, 32), -1, "i32 -1")
    _store(_MT_p, 0x123456789ABC, 64)
    _assert_eq(_load(_MT_p, 64), 0x123456789ABC, "i64 within safe int range")

    # Little-endian byte order
    _store(_MT_p, 0x1234, 16)
    _assert_eq(MEM[_MT_p], 0x34, "i16 LSB at addr+0")
    _assert_eq(MEM[_MT_p + 1], 0x12, "i16 MSB at addr+1")

    # _cstr reads up to NUL
    _MT_s = _alloc(6)
    MEM[_MT_s] = 72; MEM[_MT_s + 1] = 105; MEM[_MT_s + 2] = 0  # "Hi"
    _assert_eq(_cstr(_MT_s), "Hi", "_cstr basic")
    MEM[_MT_s] = 0
    _assert_eq(_cstr(_MT_s), "", "_cstr empty")

    # _memcpy
    _MT_dst = _alloc(4)
    _MT_src = _alloc(4)
    MEM[_MT_src] = 1; MEM[_MT_src + 1] = 2
    MEM[_MT_src + 2] = 3; MEM[_MT_src + 3] = 4
    _memcpy(_MT_dst, _MT_src, 4)
    _assert_eq(MEM[_MT_dst], 1, "_memcpy[0]")
    _assert_eq(MEM[_MT_dst + 3], 4, "_memcpy[3]")

    # _memmove with overlap going backward (dst > src);
    # source bytes are 0..7 then we move buf[0..4) into buf[2..6).
    # If we naively did a forward copy we'd see buf[5]=1 (because buf[3]
    # was already overwritten); the spec says we should see 3.
    _MT_buf = _alloc(8)
    for (_MT_i = 0; _MT_i < 8; _MT_i++) MEM[_MT_buf + _MT_i] = _MT_i
    _memmove(_MT_buf + 2, _MT_buf, 4)
    _assert_eq(MEM[_MT_buf + 2], 0, "_memmove backward overlap [2]")
    _assert_eq(MEM[_MT_buf + 5], 3, "_memmove backward overlap [5] (proves backward copy)")

    # _memset
    _MT_z = _alloc(4)
    _memset(_MT_z, 0xAB, 4)
    _assert_eq(MEM[_MT_z], 0xAB, "_memset byte value")
    _assert_eq(MEM[_MT_z + 3], 0xAB, "_memset all bytes")
    _memset(_MT_z, -1, 4)
    _assert_eq(MEM[_MT_z], 255, "_memset -1 stores 0xFF")

    # _memset_pattern
    _MT_pat = _alloc(4)
    MEM[_MT_pat] = 0xDE; MEM[_MT_pat + 1] = 0xAD
    MEM[_MT_pat + 2] = 0xBE; MEM[_MT_pat + 3] = 0xEF
    _MT_d2 = _alloc(8)
    _memset_pattern(_MT_d2, _MT_pat, 8, 4)
    _assert_eq(MEM[_MT_d2 + 0], 0xDE, "pattern [0]")
    _assert_eq(MEM[_MT_d2 + 4], 0xDE, "pattern [4] repeats")
    _assert_eq(MEM[_MT_d2 + 7], 0xEF, "pattern [7]")
}
