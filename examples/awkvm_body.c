// Demonstrates `__attribute__((annotate("awkvm_body(args):body")))`.
// awkvm reads the annotation and uses its content as the awk function
// body, skipping IR translation entirely.
//
// `clip` here is a declare-only function — no C body, awkvm provides
// the implementation through the annotation. Useful when the function
// only exists to be called from awkvm-compiled code (no native build
// to dual-target). For a dual-build pattern instead, give the C
// function a real body too — awkvm will still use the annotation,
// native compile uses the C body, and you can keep the two in sync.
//
// `(x, lo, hi)` rename list maps the awk function's parameters to
// names matching the C signature — clang -O1 strips C-source param
// names from the IR, so the rename keeps the awk body readable and
// avoids a silent name-mismatch.
//
// Body itself is multi-line via C-style adjacent string literal
// concatenation (also works in C++); for C++ specifically, raw
// string literals `R"((args):...)"` are an alternative that lets you
// drop the inner `\n`.
//
// Inputs come from argv so clang can't const-fold the `clip(...)`
// calls — important for catching mistakes in the awk body.

#define AWKVM_BODY(s) __attribute__((annotate("awkvm_body" s)))

#include <stdio.h>
#include <stdlib.h>

AWKVM_BODY(
    "(x, lo, hi):"
    "if (x < lo) return lo\n"
    "if (x > hi) return hi\n"
    "return x"
)
int clip(int x, int lo, int hi);

int main(int argc, char** argv) {
    if (argc != 6) {
        printf("usage: %s <lo> <hi> <below> <inside> <above>\n", argv[0]);
        return 1;
    }
    int lo  = atoi(argv[1]);
    int hi  = atoi(argv[2]);
    int below  = atoi(argv[3]);
    int inside = atoi(argv[4]);
    int above  = atoi(argv[5]);
    printf("clip(%d) = %d\n", below,  clip(below,  lo, hi));
    printf("clip(%d) = %d\n", inside, clip(inside, lo, hi));
    printf("clip(%d) = %d\n", above,  clip(above,  lo, hi));
    return 0;
}
