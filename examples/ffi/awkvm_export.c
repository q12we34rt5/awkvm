// Demonstrates `__attribute__((annotate("awkvm_export")))`. Compile
// with `awkvm --library` and the bare-name wrappers at the bottom of
// the emitted script let an external awk caller invoke these
// functions directly:
//
//   awkvm awkvm_export.ll --library -o lib.awk
//   gawk -f lib.awk -f awkvm_export_caller.awk
//
// Type restriction: v0.2.0 export ABI is primitive-only
// (int / long / unsigned / double / bool / char / void). Pointer or
// struct args bail at codegen with a clear message.

#define AWK_EXPORT __attribute__((annotate("awkvm_export")))

AWK_EXPORT int triangle(int n) {
    return n * (n + 1) / 2;
}

AWK_EXPORT double clipd(double x, double lo, double hi) {
    if (x < lo) return lo;
    if (x > hi) return hi;
    return x;
}

AWK_EXPORT int gcd(int a, int b) {
    while (b != 0) {
        int t = b;
        b = a % b;
        a = t;
    }
    return a < 0 ? -a : a;
}
