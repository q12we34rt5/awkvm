// Char- and line-level FILE* I/O via fputc / fputs / fgetc / fgets.
// file_io.c covers the bulk fwrite/fread path; this fixture covers
// the byte/line wrappers that share the same _stream_* primitives
// but route through different libc bridges.
//
// Round-trip:
//   write  : fputs "first line\n", then fputc 'A','B','\n'
//   read   : fgets reads back "first line\n",
//            then fgetc until EOF (-1) reads "AB\n" (3 bytes)
#include <stdio.h>

int main(int argc, char **argv) {
    if (argc != 2) return 1;
    const char *path = argv[1];

    FILE *w = fopen(path, "w");
    if (!w) return 1;
    fputs("first line\n", w);
    fputc('A', w);
    fputc('B', w);
    fputc('\n', w);
    fclose(w);

    FILE *r = fopen(path, "r");
    if (!r) return 1;
    char buf[64];
    fgets(buf, sizeof buf, r);
    printf("line1: %s", buf);

    int n = 0, c;
    while ((c = fgetc(r)) != -1) {
        n++;
        if (c == '\n') printf("nl ");
        else printf("[%c] ", c);
    }
    printf("count=%d\n", n);
    fclose(r);
    return 0;
}
