BEGIN { NEXT_ADDR = 1 }

# awkvm's runtime treats strings as byte sequences (substr / length /
# split give bytes; _ORD_TABLE maps each of 256 bytes to its
# numeric code). gawk delivers that semantics under `LC_ALL=C`; any
# multi-byte locale (UTF-8, the default on macOS / most Linux) makes
# `length(multi_byte_char) == 1`, breaking byte-level fread / fwrite,
# `cout << string`, _cstr, _str_to_mem, and inline-awk that touches
# raw bytes — silently, with bytes vanishing on the first non-ASCII
# input. Detect at startup and fail loud rather than corrupt output.
BEGIN {
    if (length("中") != 3) {
        print "awkvm: gawk is in a multi-byte locale (length(\"中\") = " length("中") \
              " ≠ 3). Byte-level I/O depends on single-byte string semantics — " \
              "rerun with `LC_ALL=C gawk -f ...`." > "/dev/stderr"
        exit 2
    }
}
