# C++ catch-clause matching. EXC_TYPE_ID holds the thrown typeinfo's
# address; PARENT_TI[ti] gives the parent under Itanium __si_class_type_info.
# When the catch clause is compatible, return EXC_TYPE_ID so the IR's
# `icmp eq selector, eh.typeid.for(catch_ti)` matches; else -1.
function _typeid_for(catch_ti,    cur) {
    cur = EXC_TYPE_ID
    while (cur != 0) {
        if (cur == catch_ti) return EXC_TYPE_ID
        cur = (cur in PARENT_TI) ? PARENT_TI[cur] : 0
    }
    return -1
}
