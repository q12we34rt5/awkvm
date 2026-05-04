const char msg[] = "abc";

__attribute__((noinline)) int at(int i) {
    return msg[i];
}

int main(void) {
    return at(1); // 'b' = 98
}
