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
