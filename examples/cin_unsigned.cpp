// `cin >> unsigned` / `cin >> unsigned long`. The token reader maps
// awk's string→number coercion through _istream_unsigned, which
// wraps high values into the awkvm signed integer model before
// _store. Round-trips through cout's unsigned overload to prove
// both directions agree on the wrap point.

#include <iostream>

int main() {
    unsigned u;
    unsigned long ul;
    std::cin >> u >> ul;
    std::cout << u << " " << ul << "\n";
    return 0;
}
