//! I/O helpers untuk userspace

use alloc::string::String;

/// Baca satu baris dari stdin
pub fn read_line() -> String {
    crate::sys::console::read_line()
}

/// Cetak ke stdout tanpa newline
pub fn print(s: &str) {
    crate::sys::console::print_fmt(format_args!("{}", s));
}

/// Cetak ke stdout dengan newline
pub fn println(s: &str) {
    crate::sys::console::print_fmt(format_args!("{}\n", s));
}
