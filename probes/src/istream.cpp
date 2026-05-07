// istream operator>> probes.
//
// Convention mirrors ostream.cpp: extern "C" + noinline so each probe
// body lowers to a single mangled call we can capture. Helpers
// (_istream_int etc.) live in runtime/iostream.awk.

#include <fstream>
#include <iostream>

#define PROBE __attribute__((noinline)) extern "C"

PROBE void awkvm_probe_istream_int(std::istream& is, int& n) {
    is >> n;
}

PROBE void awkvm_probe_istream_long(std::istream& is, long& l) {
    is >> l;
}

PROBE void awkvm_probe_istream_uint(std::istream& is, unsigned& n) {
    is >> n;
}

PROBE void awkvm_probe_istream_ulong(std::istream& is, unsigned long& n) {
    is >> n;
}

PROBE void awkvm_probe_istream_double(std::istream& is, double& d) {
    is >> d;
}

// Block / single-char unformatted input. `is.read(buf, n)` reads up
// to n bytes verbatim; `is.get()` reads one byte (or returns -1 on
// EOF). Both update `is.gcount_` (libc++'s gcount field at istream
// offset +8) so subsequent `is.gcount()` returns the right value —
// gcount() itself ISN'T probed because clang -O1 inlines it to a
// straight `_load(this+8, 64)`, no @_Z* call to capture. The store
// to MEM[stream+8] inside our helpers is what makes the inlined
// gcount() see the right value.
PROBE void awkvm_probe_istream_read(std::istream& is, char* p, long n) {
    is.read(p, n);
}

PROBE int awkvm_probe_istream_get(std::istream& is) {
    return is.get();
}

// Global-symbol probe for cin. Same shape as ostream_cerr / ostream_clog
// in ostream.cpp — the body's first @_Z* is a load of the global. Codegen
// uses STREAM_GLOBALS metadata to register the source path in the awk
// runtime's _STREAM_SRC table.
PROBE std::istream* awkvm_probe_istream_cin() {
    return &std::cin;
}

// std::ifstream constructor — same probe shape as ofstream's in
// ostream.cpp. Constructor opens the file for read; destructor (not
// probed in v0.3.0) leaves close to gawk's process-exit auto-flush.
PROBE void awkvm_probe_ifstream_ctor(const char* path) {
    std::ifstream f(path);
}
