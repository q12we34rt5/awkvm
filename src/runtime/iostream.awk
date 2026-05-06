# C++ <iostream> bridge.
#
# Each helper here is called by codegen-emitted awk in place of an
# Itanium-mangled libc++ method. The mapping from mangled name to
# helper expression lives in probes/templates.txt; build.rs uses
# probes/src/*.cpp to look up the mangled names against the user's
# actual toolchain so the binding stays correct as libc++ versions
# shift.

# Resolve the gawk output target for an ostream instance.
#
# Today this just identity-checks against the std globals: cerr / clog
# go to /dev/stderr, everything else (notably cout, plus any unrecognized
# ostream subclass) defaults to stdout.
#
# Future shape: when sstream / fstream / rdbuf swap land (E4 / E5), this
# becomes a two-level lookup — `ostream addr → streambuf id → target` —
# with the second table populated by ofstream / stringstream constructor
# probes. Keeping every _ostream_<type> routed through this one helper
# so that change is a single-file edit.
#
# Caveat: the cerr / clog symbol names hardcoded below are libc++'s
# Itanium mangling. Under libstdc++ they'd be `_ZSt4cerr` / `_ZSt4clog`,
# and dispatch would silently fall through to stdout. Cross-toolchain
# robustness is on the same TODO as the streambuf indirection.
function _ostream_dest(stream) {
    if (stream == g__ZNSt3__14cerrE) return "/dev/stderr"
    if (stream == g__ZNSt3__14clogE) return "/dev/stderr"
    return ""
}

function _ostream_int(stream, val,    d) {
    d = _ostream_dest(stream)
    if (d == "") printf "%d", val
    else         printf "%d", val > d
    return stream
}

# `cout << "literal"` lowers to libc++'s __put_character_sequence, which
# takes the byte address and a precomputed length (so no NUL scan needed).
function _ostream_cstr(stream, addr, len,    d, i) {
    d = _ostream_dest(stream)
    for (i = 0; i < len; i++) {
        if (d == "") printf "%c", MEM[addr + i]
        else         printf "%c", MEM[addr + i] > d
    }
    return stream
}

# C++ default formatting for double: 6 significant digits. gawk's %g
# matches that out of the box; precision will need a setprecision()
# manipulator path later.
function _ostream_double(stream, val,    d) {
    d = _ostream_dest(stream)
    if (d == "") printf "%g", val
    else         printf "%g", val > d
    return stream
}
