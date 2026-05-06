// The awk runtime is split across small files for readability and so that
// `cargo test` can exercise pieces in isolation. They are concatenated in a
// fixed order to form a single program prologue, prepended to every emitted
// script. Order matters only insofar as comments / function definitions read
// top-to-bottom; awk itself resolves all calls at parse time.
pub const RUNTIME: &str = concat!(
    include_str!("prelude.awk"),
    include_str!("bitwise.awk"),
    include_str!("mem.awk"),
    include_str!("str.awk"),
    include_str!("printf.awk"),
    include_str!("cxa.awk"),
    include_str!("fp.awk"),
);
