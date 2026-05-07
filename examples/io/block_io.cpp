// Block + char-level unformatted I/O round-trip.
//
// Write side:
//   ofstream::write(buf, n)  — bulk byte write, no formatting
//   ofstream::put(c)         — single byte + return stream for chain
//
// Read side:
//   ifstream::read(buf, n)   — bulk byte read up to n
//   ifstream::gcount()       — actual count from the last read
//   ifstream::get()          — single byte (or -1 on EOF)
//
// Path comes from argv so the test harness can drop it in TempDir.
//
// Verification: write 5 bytes "hello", then put '\n', then write
// 6 more bytes "world\n". Total 12 bytes. Read back via read(buf,
// 100) which should return 12 bytes (gcount() = 12); then get()
// at EOF should return -1.

#include <cstdio>
#include <fstream>

int main(int argc, char** argv) {
    if (argc != 2) return 1;
    const char* path = argv[1];

    {
        std::ofstream f(path);
        f.write("hello", 5);
        f.put('\n');
        f.write("world\n", 6);
        f.close();
    }

    char buf[100];
    long n;
    int eof;
    {
        std::ifstream g(path);
        g.read(buf, 100);
        n = g.gcount();
        buf[n] = 0;
        eof = g.get();
        g.close();
    }

    printf("read %ld bytes:\n%s", n, buf);
    printf("get-at-eof: %d\n", eof);
    return 0;
}
