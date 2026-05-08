// `std::tanh` lowers to the `llvm.tanh.f64` intrinsic at -O1 (and to
// libm's `tanh` at -O0). We expand the intrinsic to a closed form in
// terms of `exp` so neither path needs a libm bridge.
#include <cmath>
#include <cstdio>

int main() {
    std::printf("%.4f %.4f %.4f\n",
                std::tanh(0.0),
                std::tanh(1.0),
                std::tanh(-2.0));
    return 0;
}
