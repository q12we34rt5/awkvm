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
