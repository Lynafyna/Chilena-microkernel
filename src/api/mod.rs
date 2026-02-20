//! `api` — Layer abstraksi antara kernel dan userspace
//!
//! Program userspace harus menggunakan modul ini,
//! bukan akses langsung ke `sys/`.

pub mod console;
pub mod process;
pub mod syscall;
pub mod fs;
pub mod io;
