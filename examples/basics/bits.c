__attribute__((noinline)) int swap_nibbles(unsigned int x) {
    unsigned int hi = (x >> 4) & 0x0F;
    unsigned int lo = x & 0x0F;
    return (lo << 4) | hi;
}

__attribute__((noinline)) int max(int a, int b) {
    return a > b ? a : b;
}

int main(void) {
    int s = swap_nibbles(0x35);   // 0x53 = 83
    int m = max(s, 50);           // 83
    return m;
}
