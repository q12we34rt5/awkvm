# printf-family format engine. Walks the format string, pulls each
# arg from the global `_PA` array (filled inline by the caller), and
# returns the formatted string. Both `_printf` (stdout) and
# `_fprintf` (stream) sit on this. Supports %d %i %u %x %X %o %c %s
# %p %f %F %g %G %e %E and %%.
function _format(fmt_addr,    fmt, n, i, c, ai, j, conv, spec, out) {
    fmt = _cstr(fmt_addr)
    n = length(fmt)
    ai = 0
    i = 1
    out = ""
    while (i <= n) {
        c = substr(fmt, i, 1)
        if (c != "%") {
            out = out c
            i++
            continue
        }
        j = i + 1
        while (j <= n && index("diuxXocspfFgGeE%", substr(fmt, j, 1)) == 0) {
            j++
        }
        if (j > n) {
            out = out substr(fmt, i)
            break
        }
        conv = substr(fmt, j, 1)
        spec = substr(fmt, i, j - i + 1)
        if (conv == "%") {
            out = out "%"
        } else if (conv == "s") {
            out = out sprintf(spec, _cstr(_PA[ai])); ai++
        } else if (conv == "p") {
            out = out sprintf("0x%x", _PA[ai]); ai++
        } else {
            out = out sprintf(spec, _PA[ai]); ai++
        }
        i = j + 1
    }
    return out
}

function _printf(fmt_addr) {
    printf "%s", _format(fmt_addr)
    return 0
}

function _fprintf(stream, fmt_addr) {
    _stream_write_str(stream, _format(fmt_addr))
    return 0
}
