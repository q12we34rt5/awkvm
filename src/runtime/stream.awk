# Stream subsystem.
#
# Address-keyed bookkeeping for any stream (C++ ostream / istream
# instance, eventually libc FILE* and the sstream in-memory buffer).
# Five tables, all keyed by stream address — the single instance the
# user holds in their language:
#
#   _STREAM_DEST[addr]  gawk redirect target for writes ("/dev/stderr",
#                       a filename, "cmd | ..." once pipes land). "" or
#                       absent ⇒ default to stdout (bare printf).
#   _STREAM_SRC[addr]   gawk source for reads. "/dev/stdin" today;
#                       filename / pipe later.
#   _STREAM_BUF[addr]   pending input buffer for line-oriented readers
#                       that need character-level cursor (cin, getline).
#   _STREAM_POS[addr]   1-indexed cursor into _STREAM_BUF.
#   _STREAM_EOF[addr]   sticky 1 once the source returns EOF.
#
# Both the C++ iostream bridge (this file's iostream.awk neighbor) and
# the libc bridge (fopen / fread / fwrite, landing in v0.3.0) sit on
# top of these primitives, so the stream registry is a single source
# of truth regardless of which API the user code came in through.

# Append one line from the stream's source to its buffer. Returns 1 on
# success, 0 on EOF / unregistered source.
function _stream_read_line(stream,    src, line) {
    src = _STREAM_SRC[stream]
    if (src == "") return 0
    if ((getline line < src) <= 0) {
        _STREAM_EOF[stream] = 1
        return 0
    }
    _STREAM_BUF[stream] = _STREAM_BUF[stream] line "\n"
    return 1
}

# Write one byte (numeric character code) to the stream's destination.
# Empty / absent _STREAM_DEST ⇒ stdout via bare printf.
function _stream_write_byte(stream, byte,    d) {
    d = _STREAM_DEST[stream]
    if (d == "") printf "%c", byte
    else         printf "%c", byte > d
}

# Write a pre-formatted string to the stream's destination. Callers
# sprintf their format spec then hand the result here — keeps this
# helper agnostic of formatting concerns.
function _stream_write_str(stream, s,    d) {
    d = _STREAM_DEST[stream]
    if (d == "") printf "%s", s
    else         printf "%s", s > d
}
