// `cout << c` where c is `char` lowers (under libc++ -O1) to the SAME
// __put_character_sequence(stream, ptr, len) call as `cout << "literal"`,
// so the existing ostream_cstr probe handles it without a separate binding.
//
// This fixture rules out a regression where someone re-introduces an
// `awkvm_probe_ostream_char` probe — the new probe would collide on the
// same mangled name and one of the two templates would silently win,
// likely producing wrong output here. If that happens this test breaks.

#include <iostream>

int main() {
    char c = 'A';
    std::cout << c;
    return 0;
}
