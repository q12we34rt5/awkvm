// <cmath> sweep. Each variable is `volatile` so clang -O1 can't
// constant-fold the call away — the intrinsic / libc bridge has to
// actually run for the result to land in the printf args.
//
// Covers the new intrinsics (asin/acos/atan, sinh/cosh, log2/log10,
// exp2, copysign, round, minnum/maxnum), the libc bridges
// (atan2, hypot), and the FRem opcode (`std::fmod` lowers to `frem`,
// not a call).
#include <cmath>
#include <cstdio>

int main() {
    volatile double half = 0.5;
    volatile double p = 0.3;
    volatile double three = 3.0;
    volatile double four = 4.0;
    volatile double thousand = 1000.0;
    volatile double eight = 8.0;
    volatile double seven_five = 7.5;
    volatile double two = 2.0;
    volatile double minus_one = -1.0;

    std::printf("asin=%.4f acos=%.4f atan=%.4f\n",
                std::asin(half), std::acos(half), std::atan(half));
    std::printf("atan2=%.4f sinh=%.4f cosh=%.4f\n",
                std::atan2(p, half), std::sinh(half), std::cosh(half));
    std::printf("log2=%.4f log10=%.4f exp2=%.4f\n",
                std::log2(eight), std::log10(thousand), std::exp2(three));
    std::printf("copysign=%.1f round=%.1f hypot=%.4f\n",
                std::copysign(three, minus_one),
                std::round(half + two),
                std::hypot(three, four));
    std::printf("fmin=%.1f fmax=%.1f fmod=%.4f\n",
                std::fmin(half, p), std::fmax(half, p),
                std::fmod(seven_five, two));
    return 0;
}
