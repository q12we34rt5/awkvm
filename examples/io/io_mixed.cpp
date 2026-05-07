// v0.3.0 demo: same program writes via libc fopen + C++ ofstream,
// reads back via ifstream + libc fread. Proves the stream subsystem
// is shared — both API surfaces register through _stream_open_w /
// _stream_open_r on top of the same _STREAM_* tables.
//
// Two paths used per file to exercise the unification:
//   1. fopen("w") + fwrite + fclose             → file_a
//   2. ofstream(file_b) + << "..." + scope-exit → file_b
//   3. ifstream(file_a) + getline-ish via >> int → demo skipped
//      because cin >> string isn't wired (v0.4.0); we use fread
//      to read both files back.
//
// Path comes from argv[1]/argv[2] so the test harness can drop
// them in a TempDir.

#include <cstdio>
#include <fstream>

int main(int argc, char** argv) {
    if (argc != 3) return 1;
    const char* path_a = argv[1];
    const char* path_b = argv[2];

    // libc write
    FILE* fp = fopen(path_a, "w");
    fwrite("from libc\n", 1, 10, fp);
    fclose(fp);

    // C++ write
    std::ofstream f(path_b);
    f << "from ofstream\n";
    f.close();

    // Read both back via libc
    char buf[64];
    fp = fopen(path_a, "r");
    int n = (int)fread(buf, 1, 32, fp);
    buf[n] = 0;
    fclose(fp);
    printf("a: %s", buf);

    fp = fopen(path_b, "r");
    n = (int)fread(buf, 1, 32, fp);
    buf[n] = 0;
    fclose(fp);
    printf("b: %s", buf);

    return 0;
}
