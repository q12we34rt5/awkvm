// Darwin lowers `fprintf(stderr, ...)` to `fwrite(..., *__stderrp)`, so
// `__stderrp` must be allocated AND registered as a stream pointing at
// "/dev/stderr". Without that, the load returns 0 and fwrite falls
// back to plain stdout — silently misrouting every diagnostic.
//
// `check_streams` in the harness asserts both stdout and stderr
// contents, which is exactly what would catch the misrouting.
#include <stdio.h>

int main(void) {
    printf("stdout-line\n");
    fprintf(stderr, "stderr-line %d\n", 42);
    return 0;
}
