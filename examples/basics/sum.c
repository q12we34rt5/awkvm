__attribute__((noinline)) int rec_sum(int n) {
    if (n <= 0) return 0;
    return n + rec_sum(n - 1);
}

int main(void) {
    return rec_sum(5);
}
