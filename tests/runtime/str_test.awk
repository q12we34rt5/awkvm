BEGIN {
    _ST_p = _alloc(8)
    MEM[_ST_p] = 65; MEM[_ST_p + 1] = 66; MEM[_ST_p + 2] = 67; MEM[_ST_p + 3] = 0
    _assert_eq(_strlen(_ST_p), 3, "_strlen('ABC')")

    _ST_e = _alloc(1)
    MEM[_ST_e] = 0
    _assert_eq(_strlen(_ST_e), 0, "_strlen('')")

    _ST_a = _alloc(4); _ST_b = _alloc(4)
    MEM[_ST_a] = 65; MEM[_ST_a + 1] = 0
    MEM[_ST_b] = 65; MEM[_ST_b + 1] = 0
    _assert_eq(_strcmp(_ST_a, _ST_b), 0, "_strcmp equal")

    MEM[_ST_b] = 66
    _assert(_strcmp(_ST_a, _ST_b) < 0, "_strcmp 'A' < 'B'")
    _assert(_strcmp(_ST_b, _ST_a) > 0, "_strcmp 'B' > 'A'")

    # Different lengths: shorter is less when its NUL terminates first.
    _ST_x = _alloc(4); _ST_y = _alloc(4)
    MEM[_ST_x] = 65; MEM[_ST_x + 1] = 0                 # "A"
    MEM[_ST_y] = 65; MEM[_ST_y + 1] = 65; MEM[_ST_y + 2] = 0  # "AA"
    _assert(_strcmp(_ST_x, _ST_y) < 0, "_strcmp 'A' < 'AA'")

    # _atof / _atoi parse leading numeric prefix in C semantics.
    _ST_n = _alloc(8)
    MEM[_ST_n] = 49; MEM[_ST_n + 1] = 50; MEM[_ST_n + 2] = 51; MEM[_ST_n + 3] = 0  # "123"
    _assert_eq(_atoi(_ST_n), 123, "_atoi('123')")
    _assert_eq(_atof(_ST_n), 123, "_atof('123')")

    MEM[_ST_n] = 45; MEM[_ST_n + 1] = 49; MEM[_ST_n + 2] = 46
    MEM[_ST_n + 3] = 53; MEM[_ST_n + 4] = 0  # "-1.5"
    _assert_eq(_atof(_ST_n), -1.5, "_atof('-1.5')")
    _assert_eq(_atoi(_ST_n), -1, "_atoi('-1.5') truncates")
}
