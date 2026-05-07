// Demonstrates `--link helpers.awk`: a separate awk file defines
// `function fn_<name>` entries that the C side declares as `extern`
// and calls directly. clang sees the externs as ordinary unresolved
// symbols; awkvm sees them as declare-only fns and would normally
// emit no-op stubs, but the matching definitions in the linked file
// suppress that and provide the real bodies.
//
// Companion file: examples/link_basic.awk

#include <stdio.h>

extern int clip(int x, int lo, int hi);

int main(void) {
    printf("clip(  5,  0, 10) = %d\n", clip(5, 0, 10));
    printf("clip( -3,  0, 10) = %d\n", clip(-3, 0, 10));
    printf("clip( 20,  0, 10) = %d\n", clip(20, 0, 10));
    return 0;
}
