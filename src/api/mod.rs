//! `api` — Abstraction layer between kernel and userspace
//!
//! Userspace programs should use this module,
//! not direct access to `sys/`.

pub mod console;
pub mod fs;
pub mod io;
pub mod proc;
pub mod syscall;

// Re-export proc sebagai process untuk backward compatibility
pub use proc as process;
