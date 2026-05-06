function _strlen(p,    n) {
    n = 0
    while (MEM[p + n] != 0) n++
    return n
}
function _strcmp(a, b,    ca, cb) {
    while (1) {
        ca = MEM[a]; cb = MEM[b]
        if (ca != cb) return ca - cb
        if (ca == 0) return 0
        a++; b++
    }
}
# Read a NUL-terminated byte string into an awk string and let awk's
# implicit string -> number conversion do the parsing. Matches C atof
# semantics for the leading numeric prefix (sign, decimals, exponent).
function _atof(addr,    s, b) {
    s = ""
    while ((b = MEM[addr]) != 0) {
        s = s sprintf("%c", b)
        addr++
    }
    return s + 0
}
function _atoi(addr) { return int(_atof(addr)) }
# Build a C-style char** argv from awk's ARGV[]. Each ARGV[i] is copied
# byte-by-byte into MEM (NUL-terminated), and pointers are packed into
# an `argc + 1` slot table (last slot is the NULL sentinel). Requires
# LC_ALL=C so substr/length operate on bytes, not multibyte characters.
function _build_argv(    i, j, s, len, ptr, table, _ord) {
    for (i = 0; i < 256; i++) _ord[sprintf("%c", i)] = i
    table = _alloc((ARGC + 1) * 8)
    for (i = 0; i < ARGC; i++) {
        s = ARGV[i]
        len = length(s)
        ptr = _alloc(len + 1)
        for (j = 0; j < len; j++) {
            MEM[ptr + j] = _ord[substr(s, j + 1, 1)]
        }
        MEM[ptr + len] = 0
        _store(table + i * 8, ptr, 64)
    }
    _store(table + ARGC * 8, 0, 64)
    return table
}
