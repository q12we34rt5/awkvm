// Demonstrates `__attribute__((annotate("awkvm_fn(args) { body }")))`.
// awkvm reads the annotation and emits the awk function body verbatim,
// skipping IR translation entirely. The C-side type signature still
// gates callers — both `clip(x, lo, hi)` from C and inline awk
// reaching `fn_clip(...)` route through the same body.
//
// The `AWKVM_FN(decl, body)` convenience macro wraps the annotation
// attribute around a declaration; it appends `;` automatically so the
// caller doesn't need a trailing semicolon inside the macro args. The
// body uses awk function syntax (`(params) { ... }`) and reads as a
// complete awk function definition. C-style adjacent string literal
// concatenation builds it across multiple source lines; awkvm
// dedents and trims blank lines so the emitted output stays clean.
//
// `clip` here is declare-only — no C body, awkvm provides the
// implementation. For dual-build (native + awkvm), add a real C body
// after AWKVM_FN; native compile uses the C body, awkvm uses the
// annotation. The trailing `;` AWKVM_FN appends after `}` becomes a
// stray empty declaration and is silently accepted.
//
// Inputs come from argv so clang can't const-fold the `clip(...)`
// calls — important for catching mistakes in the awk body.

#define AWKVM_FN(decl, body) __attribute__((annotate("awkvm_fn" body))) decl;

#include <stdio.h>
#include <stdlib.h>

AWKVM_FN(
    int clip(int x, int lo, int hi),
    "(x, lo, hi) {"
    "    if (x < lo) return lo\n"
    "    if (x > hi) return hi\n"
    "    return x\n"
    "}"
)

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
