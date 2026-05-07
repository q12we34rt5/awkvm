// libc FILE* round-trip: write → close → reopen for read → fread →
// close → printf. Path comes from argv so the test harness can drop
// it in a TempDir.
//
// Exercises fopen / fwrite / fread / fclose layered on the v0.3.0
// stream subsystem. Two writes prove the open stream is reused
// across calls; the read-side fread requesting more bytes than the
// file holds proves the EOF return path (count returned = floor of
// bytes / size).

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    if (argc != 2) {
        puts("usage: file_io <path>");
        return 1;
    }
    const char *path = argv[1];

    FILE *fp = fopen(path, "w");
    if (!fp) { puts("fopen w failed"); return 1; }
    fwrite("hello\n", 1, 6, fp);
    fwrite("world\n", 1, 6, fp);
    fclose(fp);

    fp = fopen(path, "r");
    if (!fp) { puts("fopen r failed"); return 1; }
    char buf[64];
    memset(buf, 0, sizeof buf);
    size_t n = fread(buf, 1, 32, fp);
    fclose(fp);

    printf("read %d bytes:\n%s", (int)n, buf);
    return 0;
}
