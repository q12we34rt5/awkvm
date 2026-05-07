// Demonstrates `__asm__("AWKVM:...")` inline awk: clang sees inline
// assembly and lowers it to a `call asm` in the IR; awkvm recognizes
// the `AWKVM:` prefix and emits the body as raw awk, with `%N` operand
// placeholders substituted by the call's operand strings (output dest
// first, then inputs in declaration order).
//
// Two probes here:
//   1. Output + input — squaring an integer entirely in awk.
//   2. Three inputs, one output — multiply-add, exercising %0 / %1 /
//      %2 / %3 substitution order.
//
// On macOS arm64 clang emits a "register modifier" warning because
// the asm dialect normally wants `%w0` for 32-bit operand width.
// We don't care — awkvm treats the asm body as opaque text, the
// warning is a native-codegen concern only. Suppress so the test
// output stays clean.

#pragma clang diagnostic ignored "-Wasm-operand-widths"

#include <stdio.h>

int main(void) {
    int x = 7;
    int sq;
    __asm__("AWKVM:%0 = %1 * %1" : "=r"(sq) : "r"(x));

    int a = 3, b = 4, c = 5;
    int r;
    __asm__("AWKVM:%0 = %1 * %2 + %3" : "=r"(r) : "r"(a), "r"(b), "r"(c));

    printf("sq=%d r=%d\n", sq, r);
    return 0;
}
