// ostream operator<< probes.
//
// Each `awkvm_probe_<id>` function isolates ONE C++ operation so its lowered
// form in the resulting .ll contains exactly one externally-named call
// (the mangled libc++ symbol we want to capture). build.rs scans for
// `define void @awkvm_probe_<id>(...)` and pairs that probe id with the
// `@_Z*` call inside its body.
//
// __attribute__((noinline)) keeps the wrapper from being collapsed away
// at -O1; extern "C" gives a stable, non-mangled name we can grep for.

#include <iostream>

#define PROBE __attribute__((noinline)) extern "C"

PROBE void awkvm_probe_ostream_int(std::ostream& os, int n) {
    os << n;
}

// `os << "literal"` lowers (under libc++ -O1) to a single internal call to
// __put_character_sequence(os, ptr, len) rather than the textbook
// `operator<<(ostream&, const char*)`. We probe what actually shows up.
PROBE void awkvm_probe_ostream_cstr(std::ostream& os, const char* s) {
    os << s;
}

PROBE void awkvm_probe_ostream_double(std::ostream& os, double d) {
    os << d;
}

PROBE void awkvm_probe_ostream_long(std::ostream& os, long n) {
    os << n;
}

// Itanium signatures: `j` is unsigned int, `m` is unsigned long. Two
// separate overloads, two separate probes — but they share the same
// awk runtime helper (`_ostream_unsigned`) parameterized by bit width.
PROBE void awkvm_probe_ostream_uint(std::ostream& os, unsigned n) {
    os << n;
}

PROBE void awkvm_probe_ostream_ulong(std::ostream& os, unsigned long n) {
    os << n;
}

PROBE void awkvm_probe_ostream_bool(std::ostream& os, bool b) {
    os << b;
}

PROBE void awkvm_probe_ostream_voidptr(std::ostream& os, const void* p) {
    os << p;
}

// Two ostream operations we deliberately *don't* probe yet:
//
// * `os << c` for `char c`: libc++ lowers this to the SAME
//   __put_character_sequence(os, &c, 1) call as a string literal, so
//   the existing ostream_cstr binding handles char-via-byte already.
//   A separate probe would just collide on the mangled name.
//
// * `os << std::endl`: clang inlines endl into ~6 calls including a
//   virtual `widen('\n')` dispatched through the ctype facet's vtable.
//   Making that work requires modeling the libc++ locale machinery
//   end-to-end (or stubbing widen to identity), which is its own
//   project. Until then, write `"\n"` instead of `std::endl`.
