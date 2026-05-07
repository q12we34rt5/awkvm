int table[] = {10, 20, 30, 40};

__attribute__((noinline)) int pick(int i) {
    return table[i];
}

int main(void) {
    return pick(2) + pick(3); // 30 + 40 = 70
}
