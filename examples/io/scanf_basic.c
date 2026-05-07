// scanf reads three primitives from stdin via the v0.3.0 stream
// subsystem (lazily-registered "_scanf_stdin" sentinel stream).
// Counterpart to printf — same _PA varargs convention,
// destination addresses instead of source values.

#include <stdio.h>

int main() {
    int a;
    long b;
    double c;
    int n = scanf("%d %ld %lf", &a, &b, &c);
    printf("read %d items: a=%d b=%ld c=%g\n", n, a, b, c);
    return 0;
}
