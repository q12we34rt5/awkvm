// Capture a subprocess's stdout into a C string. awk's `cmd | getline`
// runs the command, reads one line into the variable, then close()
// flushes / releases the fd. _str_to_mem hands the awk-side buffer
// to C as a real char*.

#pragma clang diagnostic ignored "-Wasm-operand-widths"

#include <stdio.h>

int main(void) {
    char* greeting;
    __asm__(
        "AWKVM:cmd = \"printf hello\"; "
        "cmd | getline line; "
        "close(cmd); "
        "%0 = _str_to_mem(line)"
        : "=r"(greeting)
    );
    printf("subprocess said: %s\n", greeting);
    return 0;
}
