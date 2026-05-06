BEGIN {
    # f32 round-trip via memory at exact-binary values
    _FT_p = _alloc(4)
    _store_f32(_FT_p, 1.5)
    _assert_eq(_load_f32(_FT_p), 1.5, "f32 1.5")
    _store_f32(_FT_p, -3.25)
    _assert_eq(_load_f32(_FT_p), -3.25, "f32 -3.25")
    _store_f32(_FT_p, 0)
    _assert_eq(_load_f32(_FT_p), 0, "f32 0")
    _store_f32(_FT_p, 0.5)
    _assert_eq(_load_f32(_FT_p), 0.5, "f32 0.5")

    # f64 round-trip
    _FT_q = _alloc(8)
    _store_f64(_FT_q, 1.5)
    _assert_eq(_load_f64(_FT_q), 1.5, "f64 1.5")
    _store_f64(_FT_q, -3.25)
    _assert_eq(_load_f64(_FT_q), -3.25, "f64 -3.25")
    _store_f64(_FT_q, 0)
    _assert_eq(_load_f64(_FT_q), 0, "f64 0")
    _store_f64(_FT_q, 1024)
    _assert_eq(_load_f64(_FT_q), 1024, "f64 1024")

    # _f32_to_bits / _f32_from_bits direct round-trip
    _assert_eq(_f32_from_bits(_f32_to_bits(2.5)), 2.5, "f32 bits round-trip 2.5")
    _assert_eq(_f32_to_bits(0), 0, "f32_to_bits(0) = 0")

    # Subnormals collapse to 0 (Phase 8 limitation, documented in fp.awk).
    _store_f32(_FT_p, 1e-40)
    _assert_eq(_load_f32(_FT_p), 0, "f32 subnormal -> 0")
}
