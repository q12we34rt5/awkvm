// fscanf reads from a libc FILE*. fprintf writes the file first,
// then fscanf reads the same primitives back; printf round-trips
// them so the test harness can assert on stdout.

#include <stdio.h>

int main(int argc, char** argv) {
    if (argc != 2) return 1;
    const char* path = argv[1];

    FILE* fp = fopen(path, "w");
    fprintf(fp, "%d %ld %g\n", 42, 9876543210L, 3.14);
    fclose(fp);

    int a;
    long b;
    double c;
    fp = fopen(path, "r");
    int n = fscanf(fp, "%d %ld %lf", &a, &b, &c);
    fclose(fp);

    printf("read %d items: a=%d b=%ld c=%g\n", n, a, b, c);
    return 0;
}
