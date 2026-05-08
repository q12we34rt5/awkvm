// Two C++ runtime init paths exercised in one fixture:
//
//   * `Counter g_counter` has a non-trivial constructor, so clang puts
//     it in `@llvm.global_ctors`. Codegen must enumerate that table
//     and call each ctor in BEGIN — otherwise `g_counter.value` reads
//     as zero.
//
//   * `static Cached c(...)` inside `once_value()` has dynamic
//     initialization, so clang wraps the construction in
//     `__cxa_guard_acquire` / `release`. The first `acquire` call must
//     return non-zero (run me) and `release` must persist the
//     "initialized" bit; otherwise the ctor either never runs or runs
//     on every call.
#include <cstdio>

struct Counter {
    int value;
    Counter() : value(100) {}
};

static Counter g_counter;

static int g_build_count = 0;

struct Cached {
    int value;
    Cached(int x) : value(x * 7) { g_build_count++; }
};

__attribute__((noinline))
int once_value() {
    static Cached c(3);
    return c.value;
}

int main() {
    int a = once_value();
    int b = once_value();
    int c = once_value();
    std::printf("g=%d once=%d %d %d builds=%d\n",
                g_counter.value, a, b, c, g_build_count);
    return 0;
}
