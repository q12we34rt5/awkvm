__attribute__((noinline)) int risky(int x) {
    if (x < 0) throw 42;
    return x + 1;
}

int main() {
    try {
        return risky(-1);
    } catch (int e) {
        return e; // 42
    }
}
