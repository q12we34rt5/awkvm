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
# _OSTREAM_DEST is populated at codegen time in emit_globals_init's
# BEGIN block: for every external global whose mangled name matches
# an entry in build.rs's STREAM_GLOBALS table (cerr / clog today), a
# `_OSTREAM_DEST[g__<sanitized>] = "<target>"` line is emitted. cout
# and any other unrecognized stream are absent from the table and
# default to "" (gawk's stdout) via the fallback below.
#
# Future shape: when sstream / fstream / rdbuf swap land (E4 / E5),
# this becomes a two-level lookup — `ostream addr → streambuf id →
# target` — with the second table populated by ofstream /
# stringstream constructor probes. Keeping every _ostream_<type>
# routed through this one helper so that change is a single-file
# edit.
function _ostream_dest(stream) {
    return (stream in _OSTREAM_DEST) ? _OSTREAM_DEST[stream] : ""
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

# Unsigned integer print: zext from awkvm's signed integer model
# (val < 0 means the high bit was set) before formatting. `bits`
# distinguishes uint (32) from ulong (64). For ulong > 2^53 awk's
# double precision rounds — documented in LIMITATIONS.md.
function _ostream_unsigned(stream, val, bits,    d, u) {
    u = (val < 0) ? val + 2 ^ bits : val
    d = _ostream_dest(stream)
    if (d == "") printf "%d", u
    else         printf "%d", u > d
    return stream
}

# `cout << ptr` formats as "0x" + lower-case hex with no width prefix
# (libc++ default; libstdc++ may differ). Pointer values in our model
# are byte addresses, all non-negative under normal use; the zext
# guards against pointer-as-i64 with the high bit set.
function _ostream_voidptr(stream, val,    d, u) {
    u = (val < 0) ? val + 2 ^ 64 : val
    d = _ostream_dest(stream)
    if (d == "") printf "0x%x", u
    else         printf "0x%x", u > d
    return stream
}

# ============================================================
# istream / cin
# ============================================================
#
# C++ `cin >> x` is token-oriented (skip leading whitespace, read up
# to the next whitespace, leave trailing whitespace for the next
# read). gawk's `getline` is line-oriented, so we maintain a
# per-stream line buffer + cursor and refill from "/dev/stdin" when
# we run dry. cin is currently the only istream we recognize, so we
# hardcode "/dev/stdin" as the source; sstream / fstream support
# (E4 / E5) will need a streambuf-id table mirroring _OSTREAM_DEST.

# Skip whitespace at the current cursor, refilling from stdin when
# the buffer is exhausted. Returns 1 if a non-ws char is now under
# the cursor, 0 if EOF.
function _istream_skip_ws(stream,    buf, pos, line, c) {
    while (1) {
        buf = _ISTREAM_BUF[stream]
        pos = _ISTREAM_POS[stream]
        if (pos == 0) pos = 1   # awk substr is 1-indexed
        while (pos > length(buf)) {
            if ((getline line < "/dev/stdin") <= 0) {
                _ISTREAM_EOF[stream] = 1
                _ISTREAM_BUF[stream] = buf
                _ISTREAM_POS[stream] = pos
                return 0
            }
            buf = buf line "\n"
        }
        c = substr(buf, pos, 1)
        if (c != " " && c != "\t" && c != "\n") {
            _ISTREAM_BUF[stream] = buf
            _ISTREAM_POS[stream] = pos
            return 1
        }
        pos++
        _ISTREAM_BUF[stream] = buf
        _ISTREAM_POS[stream] = pos
    }
}

# Read one whitespace-delimited token. Returns "" on EOF.
function _istream_read_token(stream,    buf, pos, c, tok) {
    if (!_istream_skip_ws(stream)) return ""
    buf = _ISTREAM_BUF[stream]
    pos = _ISTREAM_POS[stream]
    tok = ""
    while (pos <= length(buf)) {
        c = substr(buf, pos, 1)
        if (c == " " || c == "\t" || c == "\n") break
        tok = tok c
        pos++
    }
    _ISTREAM_POS[stream] = pos
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
