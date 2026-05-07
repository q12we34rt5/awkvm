# C++ <iostream> bridge.
#
# Each helper here is called by codegen-emitted awk in place of an
# Itanium-mangled libc++ method. The mapping from mangled name to
# helper expression lives in probes/templates.txt; build.rs uses
# probes/src/*.cpp to look up the mangled names against the user's
# actual toolchain so the binding stays correct as libc++ versions
# shift.
#
# The address-keyed `_STREAM_*` tables (DEST / SRC / BUF / POS / EOF)
# live in stream.awk; this file is the iostream API surface that sits
# on top of them via `_stream_read_line` / `_stream_write_byte` /
# `_stream_write_str`. The libc FILE* bridge will share the same
# tables, so the stream registry is a single source of truth.

# f/i-stream open helpers. libc++'s basic_o/ifstream lays its rdbuf
# (basic_filebuf) at offset +8 from the stream object. Two addresses
# show up in IR: `<<` / `>>` use the stream's `this`, while explicit
# `close()` calls dispatch through the rdbuf at +8. Register the
# same gawk-side path against both so either path reaches the
# underlying _STREAM_* tables.
function _ofstream_open(addr, path) {
    _stream_open_w(addr, path, "file_w")
    _stream_open_w(addr + 8, path, "file_w")
}

function _ifstream_open(addr, path) {
    _stream_open_r(addr, path, "file_r")
    _stream_open_r(addr + 8, path, "file_r")
}

function _ostream_int(stream, val) {
    _stream_write_str(stream, sprintf("%d", val))
    return stream
}

# `cout << "literal"` lowers to libc++'s __put_character_sequence, which
# takes the byte address and a precomputed length (so no NUL scan needed).
function _ostream_cstr(stream, addr, len,    i) {
    for (i = 0; i < len; i++) _stream_write_byte(stream, MEM[addr + i])
    return stream
}

# C++ default formatting for double: 6 significant digits. gawk's %g
# matches that out of the box; precision will need a setprecision()
# manipulator path later.
function _ostream_double(stream, val) {
    _stream_write_str(stream, sprintf("%g", val))
    return stream
}

# Unsigned integer print: zext from awkvm's signed integer model
# (val < 0 means the high bit was set) before formatting. `bits`
# distinguishes uint (32) from ulong (64). For ulong > 2^53 awk's
# double precision rounds — documented in LIMITATIONS.md.
function _ostream_unsigned(stream, val, bits,    u) {
    u = (val < 0) ? val + 2 ^ bits : val
    _stream_write_str(stream, sprintf("%d", u))
    return stream
}

# `cout << ptr` formats as "0x" + lower-case hex with no width prefix
# (libc++ default; libstdc++ may differ). Pointer values in our model
# are byte addresses, all non-negative under normal use; the zext
# guards against pointer-as-i64 with the high bit set.
function _ostream_voidptr(stream, val,    u) {
    u = (val < 0) ? val + 2 ^ 64 : val
    _stream_write_str(stream, sprintf("0x%x", u))
    return stream
}

# ============================================================
# istream / cin
# ============================================================
#
# C++ `cin >> x` is token-oriented (skip leading whitespace, read up
# to the next whitespace, leave trailing whitespace for the next
# read). gawk's `getline` is line-oriented, so we maintain a
# per-stream line buffer + cursor (`_STREAM_BUF` / `_STREAM_POS`) and
# refill from the source registered in `_STREAM_SRC[stream]`. The
# stream tables are populated by emit_globals_init for cin (today)
# and will be extended for fstream / istringstream later.

# Skip whitespace at the current cursor, refilling from the stream's
# source when the buffer is exhausted. Returns 1 if a non-ws char is
# now under the cursor, 0 if EOF / unknown stream.
function _istream_skip_ws(stream,    buf, pos, c) {
    if (_STREAM_SRC[stream] == "") {
        _STREAM_EOF[stream] = 1
        return 0
    }
    while (1) {
        buf = _STREAM_BUF[stream]
        pos = _STREAM_POS[stream]
        if (pos == 0) pos = 1   # awk substr is 1-indexed
        while (pos > length(buf)) {
            if (!_stream_read_line(stream)) {
                _STREAM_POS[stream] = pos
                return 0
            }
            buf = _STREAM_BUF[stream]
        }
        c = substr(buf, pos, 1)
        if (c != " " && c != "\t" && c != "\n") {
            _STREAM_POS[stream] = pos
            return 1
        }
        pos++
        _STREAM_POS[stream] = pos
    }
}

# Read one whitespace-delimited token. Returns "" on EOF.
function _istream_read_token(stream,    buf, pos, c, tok) {
    if (!_istream_skip_ws(stream)) return ""
    buf = _STREAM_BUF[stream]
    pos = _STREAM_POS[stream]
    tok = ""
    while (pos <= length(buf)) {
        c = substr(buf, pos, 1)
        if (c == " " || c == "\t" || c == "\n") break
        tok = tok c
        pos++
    }
    _STREAM_POS[stream] = pos
    return tok
}

# `cin >> int_var` — read one token, awk's string→number coercion
# parses the leading numeric prefix (matching strtol's behavior),
# then store at the destination address. `bits` distinguishes int /
# long / etc. so the same helper can serve both.
function _istream_int(stream, dest, bits,    tok) {
    tok = _istream_read_token(stream)
    _store(dest, tok + 0, bits)
    return stream
}

# `cin >> double_var` — same token-read; awk's coercion handles the
# decimal / exponent forms strtod does. Storage is IEEE 754 via the
# fp.awk pack helpers.
function _istream_double(stream, dest,    tok) {
    tok = _istream_read_token(stream)
    _store_f64(dest, tok + 0)
    return stream
}

# `cin >> unsigned_var` — token-read, awk parses the leading numeric
# prefix as a non-negative number. _store sign-extends, so values
# above 2^(bits-1) wrap to negative in the awkvm signed model — same
# convention as _ostream_unsigned uses on the read side.
function _istream_unsigned(stream, dest, bits,    tok, v, lim) {
    tok = _istream_read_token(stream)
    v = tok + 0
    lim = 2 ^ (bits - 1)
    if (v >= lim) v -= 2 ^ bits
    _store(dest, v, bits)
    return stream
}
