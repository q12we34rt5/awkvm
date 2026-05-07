__attribute__((noinline)) double mul(double a, double b) {
    return a * b;
}

struct Box {
    double v;
};

__attribute__((noinline)) double get(struct Box *b) {
    return b->v;
}

int main(void) {
    double r = mul(3.5, 4.0);   // 14.0
    struct Box b = {0.5};
    r = r + get(&b);            // 14.5
    if (r > 14.0 && r < 15.0) {
        return (int)r;          // 14
    }
    return -1;
}
