struct Base {
    int tag;
    Base(int t) : tag(t) {}
};

struct Derived : Base {
    Derived(int t) : Base(t) {}
};

__attribute__((noinline)) void risky(int x) {
    if (x < 0) throw Derived{42};
}

int main() {
    try {
        risky(-1);
    } catch (const Base &b) {
        return b.tag; // 42
    }
    return 0;
}
