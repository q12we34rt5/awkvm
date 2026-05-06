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
