# scanf-family format engine. Walks `fmt`, pulls destination
# addresses from the global `_PA` array (filled by the caller before
# invocation), reads tokens from the stream, and stores parsed
# values at each destination. Returns the count of items
# successfully assigned (matching the C scanf return contract).
#
# Supported specs: %d %i %ld (signed int); %u %x %X %o %lu (unsigned
# wrapped to awkvm's signed model); %f %lf %g %lg %e %le (float
# stored via _store_f32 / _store_f64 by length modifier); %s
# (whitespace-delimited C-string copy with NUL terminator); %c
# (single byte, no whitespace skip). Width specifiers and `*`
# assignment-suppression are ignored.
function _scanf_engine(stream, fmt_addr,
                       fmt, n, i, ai, count, j, conv, is_long, addr, tok, b, k, v, bits, lim) {
    fmt = _cstr(fmt_addr)
    n = length(fmt)
    ai = 0
    count = 0
    i = 1
    while (i <= n) {
        if (substr(fmt, i, 1) != "%") {
            i++
            continue   # literal / whitespace in fmt: scanf semantics treat
                       # both as "implicit whitespace skip", which our
                       # token-reader already does on every numeric / %s
                       # spec — so dropping these matches the dominant
                       # cases (`"%d %d"`, `"%d,%d"`, `"%lf %lf"`).
        }
        j = i + 1
        is_long = 0
        if (j <= n && substr(fmt, j, 1) == "l") { is_long = 1; j++ }
        if (j > n) break
        conv = substr(fmt, j, 1)
        addr = _PA[ai]; ai++
        bits = is_long ? 64 : 32
        if (conv == "d" || conv == "i") {
            tok = _istream_read_token(stream)
            if (tok == "") break
            _store(addr, tok + 0, bits)
            count++
        } else if (conv == "u" || conv == "x" || conv == "X" || conv == "o") {
            tok = _istream_read_token(stream)
            if (tok == "") break
            v = tok + 0
            lim = 2 ^ (bits - 1)
            if (v >= lim) v -= 2 ^ bits
            _store(addr, v, bits)
            count++
        } else if (conv == "f" || conv == "g" || conv == "e") {
            tok = _istream_read_token(stream)
            if (tok == "") break
            if (is_long) _store_f64(addr, tok + 0)
            else         _store_f32(addr, tok + 0)
            count++
        } else if (conv == "s") {
            tok = _istream_read_token(stream)
            if (tok == "") break
            for (k = 0; k < length(tok); k++) {
                MEM[addr + k] = _ORD_TABLE[substr(tok, k + 1, 1)]
            }
            MEM[addr + length(tok)] = 0
            count++
        } else if (conv == "c") {
            b = _stream_read_byte(stream)
            if (b < 0) break
            _store(addr, b, 8)
            count++
        }
        i = j + 1
    }
    return count
}

# scanf — reads from /dev/stdin via a sentinel-keyed stream record.
# Lazy-register on first call so the stream tables stay clean for
# programs that don't use scanf.
function _scanf(fmt_addr) {
    if (!("_scanf_stdin" in _STREAM_SRC)) {
        _STREAM_SRC["_scanf_stdin"] = "/dev/stdin"
        _STREAM_KIND["_scanf_stdin"] = "file_r"
    }
    return _scanf_engine("_scanf_stdin", fmt_addr)
}

function _fscanf(stream, fmt_addr) {
    return _scanf_engine(stream, fmt_addr)
}

# sscanf — input source is a NUL-terminated C-string in MEM, not a
# stream. Preload `_STREAM_BUF` for a sentinel key and run the same
# engine; with no `_STREAM_SRC` registered, `_stream_read_line` will
# return 0 once the buffer is consumed and the engine stops.
function _sscanf(addr, fmt_addr,    key) {
    key = "_sscanf_buf"
    _STREAM_BUF[key] = _cstr(addr)
    _STREAM_POS[key] = 1
    delete _STREAM_SRC[key]
    delete _STREAM_EOF[key]
    return _scanf_engine(key, fmt_addr)
}
