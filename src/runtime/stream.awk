# Stream subsystem.
#
# Address-keyed bookkeeping for any stream (C++ ostream / istream
# instance, libc FILE*, popen pipe, eventually sstream's in-memory
# buffer). Six tables, all keyed by stream address — the single
# instance the user holds in their language:
#
#   _STREAM_DEST[addr]  gawk redirect target for writes (file path,
#                       pipe command, "/dev/stderr"). "" or absent
#                       ⇒ default to stdout (bare printf).
#   _STREAM_SRC[addr]   gawk source for reads (file path or pipe
#                       command).
#   _STREAM_KIND[addr]  routing tag — selects the gawk redirect
#                       operator at write/read time:
#                         "file_w"  → `printf ... > path`
#                         "file_a"  → `printf ... >> path`
#                         "file_r"  → `getline ... < path`
#                         "pipe_w"  → `printf ... | cmd`
#                         "pipe_r"  → `cmd | getline ...`
#                       Absent ⇒ defaults match cin / cerr / clog
#                       (file_r / file_w over /dev/std*).
#   _STREAM_BUF[addr]   pending input buffer for line-oriented readers
#                       that need character-level cursor (cin, fread,
#                       fgets).
#   _STREAM_POS[addr]   1-indexed cursor into _STREAM_BUF.
#   _STREAM_EOF[addr]   sticky 1 once the source returns EOF.
#
# Both the C++ iostream bridge (iostream.awk) and the libc bridge
# (libc.awk) sit on top of these primitives, so the stream registry
# is a single source of truth regardless of which API the user code
# came in through.

# Register a write-mode stream. `target` is the gawk redirect
# destination (file path or pipe command); `kind` picks the operator
# (`file_w` / `file_a` / `pipe_w`).
function _stream_open_w(addr, target, kind) {
    _STREAM_DEST[addr] = target
    _STREAM_KIND[addr] = kind
}

# Register a read-mode stream. `kind` is `file_r` or `pipe_r`.
function _stream_open_r(addr, target, kind) {
    _STREAM_SRC[addr]  = target
    _STREAM_KIND[addr] = kind
}

# Close the underlying gawk handle and drop all per-stream state.
# Returns the gawk close() result so pipe streams can surface the
# child process's exit status (libc pclose semantics).
function _stream_close(addr,    target, status) {
    target = _STREAM_DEST[addr]
    if (target == "") target = _STREAM_SRC[addr]
    status = (target != "") ? close(target) : 0
    delete _STREAM_DEST[addr]
    delete _STREAM_SRC[addr]
    delete _STREAM_KIND[addr]
    delete _STREAM_BUF[addr]
    delete _STREAM_POS[addr]
    delete _STREAM_EOF[addr]
    return status
}

# Append one line from the stream's source to its buffer. Returns 1
# on success, 0 on EOF / unregistered source. `pipe_r` streams use
# `cmd | getline`; everything else falls back to `getline < path`.
function _stream_read_line(stream,    src, k, line, r) {
    src = _STREAM_SRC[stream]
    if (src == "") return 0
    k = _STREAM_KIND[stream]
    if (k == "pipe_r") r = (src | getline line)
    else               r = (getline line < src)
    if (r <= 0) {
        _STREAM_EOF[stream] = 1
        return 0
    }
    _STREAM_BUF[stream] = _STREAM_BUF[stream] line "\n"
    return 1
}

# Pull one byte (numeric character code) from the buffered reader.
# Returns -1 on EOF / unregistered source. Refills the line buffer
# as needed.
function _stream_read_byte(stream,    buf, pos, c) {
    while (1) {
        buf = _STREAM_BUF[stream]
        pos = _STREAM_POS[stream]
        if (pos == 0) pos = 1
        if (pos <= length(buf)) {
            c = substr(buf, pos, 1)
            _STREAM_POS[stream] = pos + 1
            return _ORD_TABLE[c]
        }
        if (!_stream_read_line(stream)) return -1
    }
}

# Write one byte (numeric character code) to the stream's destination.
function _stream_write_byte(stream, byte,    d, k) {
    d = _STREAM_DEST[stream]
    k = _STREAM_KIND[stream]
    if (k == "pipe_w")      printf "%c", byte | d
    else if (k == "file_a") printf "%c", byte >> d
    else if (d != "")       printf "%c", byte > d
    else                    printf "%c", byte
}

# Write a pre-formatted string to the stream's destination. Callers
# sprintf their format spec then hand the result here — keeps this
# helper agnostic of formatting concerns.
function _stream_write_str(stream, s,    d, k) {
    d = _STREAM_DEST[stream]
    k = _STREAM_KIND[stream]
    if (k == "pipe_w")      printf "%s", s | d
    else if (k == "file_a") printf "%s", s >> d
    else if (d != "")       printf "%s", s > d
    else                    printf "%s", s
}
