// Type punning: write i32, read individual bytes back.
__attribute__((noinline)) int low_byte(unsigned char *buf, int v) {
    int *p = (int *)buf;
    *p = v;
    return buf[0];
}

int main(void) {
    unsigned char buf[4];
    return low_byte(buf, 0x12345678); // 0x78 = 120
}
