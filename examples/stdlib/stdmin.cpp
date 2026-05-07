#include <algorithm>

__attribute__((noinline)) int pick(int a, int b) {
    return std::min(a, b);
}

int main() {
    return pick(7, 3); // 3
}
