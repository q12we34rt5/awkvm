// Demonstrates the C ↔ awk-string marshal both directions through
// inline awk:
//
//   * `_cstr(addr)`        — MEM byte address → awk string
//                            (already in runtime/mem.awk)
//   * `_str_to_mem(s)`     — awk string → MEM allocation, returns
//                            base address as a C-style char*
//                            (in runtime/str.awk)
//
// The example runs `toupper` on a C string entirely in awk-land,
// then hands the uppercased buffer back to C via an `"=r"` output
// operand. Same template works for any awk-only computation that
// produces a string (regex sub/gsub, sprintf, pipe-from-subprocess,
// etc.).

#pragma clang diagnostic ignored "-Wasm-operand-widths"

#include <stdio.h>

int main(void) {
    const char* input = "Hello, World";
    char* upper;
    __asm__(
        "AWKVM:s = _cstr(%1); "
        "s = toupper(s); "
        "%0 = _str_to_mem(s)"
        : "=r"(upper)
        : "r"(input)
    );
    printf("%s\n", upper);
    return 0;
}
