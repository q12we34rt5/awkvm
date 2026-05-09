// popen("r") opens a child process and captures its stdout via
// gawk's `cmd | getline`. fgets pulls one line at a time; pclose
// surfaces the child exit status via gawk's close() return value.
// Write mode ("w") feeds the child stdin via `print | cmd` and is
// symmetric — covered indirectly by the same _stream_open_w path.
//
// `printf '...'` (POSIX shell builtin) is portable across mac and
// linux; the doubled backslashes survive C string escaping so the
// shell sees `printf 'hello\nworld\n'`.
#include <stdio.h>

int main(void) {
    FILE *fp = popen("printf 'hello\\nworld\\n'", "r");
    if (!fp) return 1;
    char line[64];
    int n = 0;
    while (fgets(line, sizeof line, fp)) {
        printf("line%d: %s", ++n, line);
    }
    int rc = pclose(fp);
    printf("rc=%d count=%d\n", rc, n);
    return 0;
}
