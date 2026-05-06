// Reads N then N integers from stdin, prints n / sum / min / max / mean.
// Exercises cin >> int (loop), cout << with mixed types (string / long /
// double), cerr for error path, conditional exit code. Standalone-style
// fixture rather than a feature regression — meant as a "this is what
// awkvm produces today" demo.

#include <iostream>

int main() {
    int n;
    std::cin >> n;
    if (n <= 0) {
        std::cerr << "error: n must be positive\n";
        return 1;
    }

    long sum = 0;
    int mn = 0, mx = 0;
    for (int i = 0; i < n; i++) {
        int x;
        std::cin >> x;
        sum += x;
        if (i == 0) {
            mn = x;
            mx = x;
        } else {
            if (x < mn) mn = x;
            if (x > mx) mx = x;
        }
    }
    double mean = (double) sum / n;

    std::cout << "n=" << n
              << " sum=" << sum
              << " min=" << mn
              << " max=" << mx
              << " mean=" << mean << "\n";
    return 0;
}
