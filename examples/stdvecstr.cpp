#include <vector>
#include <string>

__attribute__((noinline)) int total(int n) {
    std::vector<std::string> v;
    for (int i = 0; i < n; i++) v.push_back("hi");
    int t = 0;
    for (auto &s : v) t += s.size();
    return t;
}

int main() {
    return total(5); // 5 * 2 = 10
}
