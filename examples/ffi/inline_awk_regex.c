// gawk's regex (`/pattern/`, `gsub`, `sub`, `match`) reachable from C
// via inline awk. Here gsub replaces every `o` with `0` and the
// modified awk string is marshaled back to C as a char*.
//
// The same idiom covers any awk-only string transform — `toupper`,
// `tolower`, `sprintf("%-20s", x)`, `substr(s, a, b)`, etc.

#pragma clang diagnostic ignored "-Wasm-operand-widths"

#include <stdio.h>

int main(void) {
    const char* in = "hello world";
    char* out;
    __asm__(
        "AWKVM:s = _cstr(%1); "
        "gsub(/o/, \"0\", s); "
        "%0 = _str_to_mem(s)"
        : "=r"(out)
        : "r"(in)
    );
    printf("%s\n", out);
    return 0;
}
