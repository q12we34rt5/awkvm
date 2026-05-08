// Darwin libc inlines isalpha / isdigit / etc. as a load from
// `_DefaultRuneLocale.__runetype[c] & FLAG`. The struct is an external
// global in the IR, so the runtime has to allocate it AND populate the
// ASCII range — otherwise every ctype call returns 0 and any input
// parser (argument parsing, std::stoi, JSON, CSV) silently degrades.
#include <ctype.h>
#include <stdio.h>

int main(void) {
    int hits = 0;
    hits += isalpha('s')  ? 1 : 0;   // 1
    hits += isalpha('-')  ? 0 : 1;   // 1 (correctly NOT alpha)
    hits += isdigit('5')  ? 1 : 0;   // 1
    hits += isdigit(' ')  ? 0 : 1;   // 1 (correctly NOT digit)
    hits += isspace(' ')  ? 1 : 0;   // 1
    hits += isspace('\t') ? 1 : 0;   // 1
    hits += isupper('S')  ? 1 : 0;   // 1
    hits += islower('s')  ? 1 : 0;   // 1
    hits += ispunct('!')  ? 1 : 0;   // 1
    hits += isxdigit('a') ? 1 : 0;   // 1
    printf("hits=%d\n", hits);
    return 0;
}
