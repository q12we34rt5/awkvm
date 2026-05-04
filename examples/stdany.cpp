#include <any>

__attribute__((noinline)) int unwrap(std::any a) {
    return std::any_cast<int>(a);
}

int main() {
    std::any a = 42;
    return unwrap(a); // 42
    
}
