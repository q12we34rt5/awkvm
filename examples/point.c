struct Point {
    int x;
    int y;
};

__attribute__((noinline)) int dot(struct Point *p, struct Point *q) {
    return p->x * q->x + p->y * q->y;
}

int main(void) {
    struct Point a = {3, 4};
    struct Point b = {5, 6};
    return dot(&a, &b); // 3*5 + 4*6 = 39
}
