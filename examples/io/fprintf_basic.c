// fprintf to a libc FILE* — counterpart to printf, routed through
// the v0.3.0 stream subsystem via _fprintf(stream, fmt). Same
// format engine as _printf, just routed via _stream_write_str
// instead of bare gawk printf.
//
// Path comes from argv so the test harness drops it in TempDir.

#include <stdio.h>

int main(int argc, char** argv) {
    if (argc != 2) return 1;
    const char* path = argv[1];

    FILE* fp = fopen(path, "w");
    fprintf(fp, "n=%d s=%s pi=%.3f\n", 42, "hello", 3.14159);
    fclose(fp);

    char buf[128];
    fp = fopen(path, "r");
    int n = (int)fread(buf, 1, sizeof buf - 1, fp);
    buf[n] = 0;
    fclose(fp);
    printf("got %d bytes: %s", n, buf);
    return 0;
}
