// ifstream + `>>` extraction operator. Proves the v0.3.0 stream
// subsystem covers the read direction for primitive types: ifstream's
// `>>` resolves through the same mangled overloads as `cin >>`
// (basic_istream::operator>> for int / long / double), so the
// existing _istream_* probes catch it via base-class inheritance.
//
// What this *doesn't* exercise (deferred to v0.4.0):
//   - `is.read(buf, n)` / `is.gcount()` block-read methods
//   - `is >> std::string` / `std::getline(is, str)` string ops
//   - `is.eof()` / `is.fail()` / `is.good()` state queries
//
// Round-trip: ofstream writes three primitives space-separated,
// closes; ifstream reopens, extracts them in order, prints via
// printf for the harness to assert on. Path lives in a TempDir.

#include <cstdio>
#include <fstream>

int main(int argc, char** argv) {
    if (argc != 2) return 1;
    const char* path = argv[1];

    {
        std::ofstream f(path);
        f << 42 << " " << 3.14 << " " << 1234567890 << "\n";
        f.close();
    }

    int a;
    double b;
    long c;
    {
        std::ifstream g(path);
        g >> a >> b >> c;
        g.close();
    }

    printf("a=%d b=%g c=%ld\n", a, b, c);
    return 0;
}
