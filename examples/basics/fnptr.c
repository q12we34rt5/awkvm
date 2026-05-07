__attribute__((noinline)) int twice(int x) { return x * 2; }
__attribute__((noinline)) int neg(int x) { return -x; }

__attribute__((noinline)) int apply(int (*f)(int), int x) {
    return f(x);
}

int main(void) {
    return apply(twice, 7) + apply(neg, 3); // 14 + (-3) = 11
}
