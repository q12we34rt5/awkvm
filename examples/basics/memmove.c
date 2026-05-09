// memmove handles overlap by picking copy direction from dst vs
// src — forward when dst < src, backward otherwise. memcpy on
// most platforms gives wrong results for the overlapping case (and
// may even diagnose it). awkvm's _memmove walks the right
// direction in one pass.
//
// At -O1 clang would otherwise fold the 8-byte memmove into a
// single i64 load+store and never call our _memmove at all (the
// 8-byte alias triggers awkvm's i64-via-double precision loss
// near 2^63 — interesting bug, but a different one). Reading the
// size through a `volatile` global hides the value from clang's
// interprocedural constant propagation, so the actual library
// call survives.
//
//   right shift: memmove(buf+2, buf, 8)  — dst > src, walks backward
//   left shift : memmove(buf, buf+2, 8)  — dst < src, walks forward
#include <stdio.h>
#include <string.h>

volatile size_t g_n = 8;

int main(void) {
    char buf[16] = "abcdefghij";       // 10 chars + NUL
    memmove(buf + 2, buf, g_n);        // -> "ababcdefgh"
    printf("right: %s\n", buf);

    char buf2[16] = "ABCDEFGHIJ";
    memmove(buf2, buf2 + 2, g_n);      // -> "CDEFGHIJIJ"
    printf("left: %s\n", buf2);

    return 0;
}
