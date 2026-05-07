// sscanf parses a C-string in memory. Preloads _STREAM_BUF directly
// (no SRC registered), so the scanf engine consumes from the buffer
// and stops when exhausted.

#include <stdio.h>

int main() {
    const char* input = "42 9876543210 3.14 hello";
    int a;
    long b;
    double c;
    char s[32];
    int n = sscanf(input, "%d %ld %lf %s", &a, &b, &c, s);
    printf("read %d items: a=%d b=%ld c=%g s=%s\n", n, a, b, c, s);
    return 0;
}
