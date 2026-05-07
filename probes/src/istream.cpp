// istream operator>> probes.
//
// Convention mirrors ostream.cpp: extern "C" + noinline so each probe
// body lowers to a single mangled call we can capture. Helpers
// (_istream_int etc.) live in runtime/iostream.awk.

#include <iostream>

#define PROBE __attribute__((noinline)) extern "C"

PROBE void awkvm_probe_istream_int(std::istream& is, int& n) {
    is >> n;
}

PROBE void awkvm_probe_istream_long(std::istream& is, long& l) {
    is >> l;
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
