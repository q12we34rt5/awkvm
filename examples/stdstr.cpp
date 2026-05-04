#include <string>

__attribute__((noinline)) int len(const char *s) {
    std::string str(s);
    return str.size();
}

int main() {
    return len("hello, awkvm"); // 12
}
