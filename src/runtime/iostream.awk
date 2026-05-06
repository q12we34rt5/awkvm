# C++ <iostream> bridge.
#
# Each helper here is called by codegen-emitted awk in place of an
# Itanium-mangled libc++ method. The mapping from mangled name to
# helper expression lives in probes/templates.txt; build.rs uses
# probes/src/*.cpp to look up the mangled names against the user's
# actual toolchain so the binding stays correct as libc++ versions
# shift.
#
# For now we treat every ostream as if it were stdout — `cout` vs
# `cerr` vs `clog` distinction is on the TODO list, but the only
# fixture today only writes to stdout.

function _ostream_int(stream, val) {
    printf "%d", val
    return stream
}
