#include <vector>

__attribute__((noinline)) int sum(int n) {
    std::vector<int> v;
    for (int i = 0; i < n; i++) v.push_back(i);
    int s = 0;
    for (int x : v) s += x;
    return s;
}

int main() {
    return sum(5); // 0+1+2+3+4 = 10
}
