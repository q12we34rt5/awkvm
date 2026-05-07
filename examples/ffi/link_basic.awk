# Linked awk helper for examples/link_basic.c.
# The `fn_` prefix matches awkvm's internal naming convention so the
# C-side `extern int clip(...)` resolves through the standard
# fn_<name> codegen path.

function fn_clip(x, lo, hi) {
    if (x < lo) return lo
    if (x > hi) return hi
    return x
}
