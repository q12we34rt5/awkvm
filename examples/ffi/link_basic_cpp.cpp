// C++ counterpart of link_basic.c — exercises the `extern "C"` wrap
// that `--link` callers need in C++ source. Without it, clang++
// mangles `clip(int, int, int)` to `_Z4clipiii` and awkvm's
// `fn_<sanitize>` rule emits `fn__Z4clipiii`, which doesn't match the
// linked `fn_clip` definition — the call falls through to a no-op
// stub returning 0. With it, the symbol stays `clip` in IR and
// resolves through the standard `fn_clip` path.
//
// Shares examples/link_basic.awk as the helper file; the test
// invocation passes the same `--link` arg as the C version.

#include <stdio.h>

extern "C" int clip(int x, int lo, int hi);

int main() {
    printf("clip(  5,  0, 10) = %d\n", clip(5, 0, 10));
    printf("clip( -3,  0, 10) = %d\n", clip(-3, 0, 10));
    printf("clip( 20,  0, 10) = %d\n", clip(20, 0, 10));
    return 0;
}
