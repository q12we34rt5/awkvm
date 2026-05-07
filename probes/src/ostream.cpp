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

#include <fstream>
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

// Block / single-char unformatted output. `os.write(buf, n)` is the
// counterpart of fwrite — emit n bytes verbatim with no formatting.
// `os.put(c)` writes one byte. Both return the ostream so chained
// `os.write(...).put(...)` works.
PROBE void awkvm_probe_ostream_write(std::ostream& os, const char* p, long n) {
    os.write(p, n);
}

PROBE void awkvm_probe_ostream_put(std::ostream& os, char c) {
    os.put(c);
}

// Global-symbol probes. Different shape from the call probes above:
// the body's first @_Z* reference is a global *load* (or ret with a
// global address), not a call. build.rs uses the same parser to
// extract the mangled name; templates.txt routes these probe_ids
// through the `:=` sigil so they end up in STREAM_GLOBALS instead of
// PROBE_MAP. Codegen consults STREAM_GLOBALS in emit_globals_init to
// register the stream's output target in the awk runtime's
// _STREAM_DEST table.
PROBE std::ostream* awkvm_probe_ostream_cerr() {
    return &std::cerr;
}

PROBE std::ostream* awkvm_probe_ostream_clog() {
    return &std::clog;
}

// std::ofstream constructor — `std::ofstream f(path)` lowers to a
// stack alloca + a call to the C1 constructor (`...C1EPKcj`) with
// the openmode flag defaulting to `out`. The runtime template
// registers the path as a write-mode stream against the ofstream's
// `this` pointer (arg0); subsequent `<<` operations resolve through
// the same _ostream_* helpers as cout because they're inherited
// from std::ostream. Mode flags (app / trunc / ate / binary) are
// ignored in v0.3.0 — default truncate-on-open matches the
// dominant `ofstream f(path)` usage.
//
// Destructor handling: the libc++ destructor is virtual and
// linkonce_odr-defined in the user's IR, so the translated awk
// body runs at scope exit; it manipulates vtables / locale state
// that awkvm doesn't model, but in practice that no-ops out. The
// file is closed by gawk's auto-flush at process exit. For
// long-lived programs that need explicit mid-program close, call
// `f.close()` (which routes through a libc++ method we don't yet
// probe — TODO for v0.4.0).
PROBE void awkvm_probe_ofstream_ctor(const char* path) {
    std::ofstream f(path);
}

// basic_filebuf::close — covers explicit `f.close()` on both
// ofstream and ifstream, plus the filebuf-close call inside the
// linkonce_odr destructor body. Without intercepting close at this
// level, gawk's redirected output stays buffered (and read-after-
// write to the same file in one program reads stale state).
PROBE void awkvm_probe_filebuf_close(std::filebuf* f) {
    f->close();
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
