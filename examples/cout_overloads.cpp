// Exercises the second batch of ostream operator<< overloads:
//   long, unsigned, unsigned long, bool, const void*.
// `unsigned` is past i32 max (4000000000 > 2^31-1) on purpose so the
// test fails if zext logic in _ostream_unsigned regresses.

#include <iostream>

int main() {
    long l = 1234567890L;
    unsigned u = 4000000000U;
    unsigned long ul = 12345UL;
    bool b = true;
    void* p = (void*)0x42;
    std::cout << l << " " << u << " " << ul << " " << b << " " << p << "\n";
    return 0;
}
