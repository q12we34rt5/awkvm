struct Quad {
    int a, b, c, d;
};

__attribute__((noinline)) struct Quad make(int x) {
    struct Quad q = {x, x + 1, x + 2, x + 3};
    return q;
}

int main(void) {
    struct Quad q = make(10);
    return q.a + q.b + q.c + q.d; // 10 + 11 + 12 + 13 = 46
}
