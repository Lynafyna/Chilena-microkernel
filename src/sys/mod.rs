//! sys — Chilena kernel subsystems
//!
//! Struktur:
//!   arch/    — x86_64: gdt, idt, cpu, pic
//!   proc/    — process management: process, sched
//!   power/   — power management: acpi
//!   debug/   — debugging: serial
//!   ipc/     — inter-process communication
//!   mem/     — memory management
//!   fs/      — filesystem
//!   syscall/ — syscall dispatcher

// Grouped modules (baru)
pub mod arch;
pub mod proc;
pub mod power;
pub mod debug;
pub mod ipc;

// Re-export untuk backward compatibility
// Semua path lama seperti sys::gdt::, sys::process::, dll tetap jalan
pub use arch::gdt;
pub use arch::idt;
pub use arch::cpu;
pub use arch::pic;
pub use proc::process;
pub use proc::sched;
pub use power::acpi;
pub use debug::serial;

// Modules yang belum dikelompok
pub mod clk;
pub mod console;
pub mod fs;
pub mod keyboard;
pub mod mem;
pub mod pci;
pub mod syscall;
pub mod vga;
pub mod virtio;
