# Walks the format string, forwarding each spec to gawk's printf and pulling
# args from the global _PA array (filled by the caller before invocation).
# Supports %d %i %u %x %X %o %c %s %p and %% (no float specifiers yet).
function _printf(fmt_addr,    fmt, n, i, c, ai, j, conv, spec) {
    fmt = _cstr(fmt_addr)
    n = length(fmt)
    ai = 0
    i = 1
    while (i <= n) {
        c = substr(fmt, i, 1)
        if (c != "%") {
            printf "%s", c
            i++
            continue
        }
        j = i + 1
        while (j <= n && index("diuxXocspfFgGeE%", substr(fmt, j, 1)) == 0) {
            j++
        }
        if (j > n) {
            printf "%s", substr(fmt, i)
            break
        }
        conv = substr(fmt, j, 1)
        spec = substr(fmt, i, j - i + 1)
        if (conv == "%") {
            printf "%%"
        } else if (conv == "s") {
            printf spec, _cstr(_PA[ai]); ai++
        } else if (conv == "p") {
            printf "0x%x", _PA[ai]; ai++
        } else {
            printf spec, _PA[ai]; ai++
        }
        i = j + 1
    }
    return 0
}
