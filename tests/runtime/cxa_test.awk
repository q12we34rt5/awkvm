BEGIN {
    # No exception in flight: nothing matches.
    EXC_TYPE_ID = 0
    delete PARENT_TI
    _assert_eq(_typeid_for(100), -1, "no exception -> -1")

    # Exact match returns thrown typeinfo address (not the catch's).
    EXC_TYPE_ID = 100
    _assert_eq(_typeid_for(100), 100, "identity match")

    # Single-inheritance: catch (Base) accepts thrown Derived.
    EXC_TYPE_ID = 200
    PARENT_TI[200] = 100
    _assert_eq(_typeid_for(100), 200, "parent match returns thrown id")
    _assert_eq(_typeid_for(200), 200, "self match")
    _assert_eq(_typeid_for(999), -1, "unrelated -> -1")

    # Two-level chain.
    PARENT_TI[300] = 200  # 300 : 200 : 100
    EXC_TYPE_ID = 300
    _assert_eq(_typeid_for(100), 300, "grandparent match")
    _assert_eq(_typeid_for(200), 300, "parent (2 levels)")
    _assert_eq(_typeid_for(300), 300, "self")
}
