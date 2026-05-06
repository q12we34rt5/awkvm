# Shared assertion helpers for runtime/*_test.awk.
# Test files put their work in BEGIN blocks; this file's END runs last and
# turns the failure count into the gawk exit status.
function _assert(cond, msg) {
    _TESTS_TOTAL++
    if (!cond) {
        printf "FAIL: %s\n", msg > "/dev/stderr"
        _TESTS_FAILED++
    }
}
function _assert_eq(actual, expected, msg) {
    _TESTS_TOTAL++
    if (actual != expected) {
        printf "FAIL: %s — got %s, expected %s\n", msg, actual, expected > "/dev/stderr"
        _TESTS_FAILED++
    }
}
END {
    if (_TESTS_FAILED + 0 > 0) {
        printf "%d/%d runtime tests failed\n", _TESTS_FAILED, _TESTS_TOTAL > "/dev/stderr"
        exit 1
    }
    printf "%d runtime tests passed\n", _TESTS_TOTAL
}
