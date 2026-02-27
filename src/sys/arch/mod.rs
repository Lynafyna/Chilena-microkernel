//! arch — x86_64 hardware abstraction layer
//!
//! Berisi semua komponen yang berhubungan langsung dengan arsitektur x86_64:
//!   - GDT (Global Descriptor Table)
//!   - IDT (Interrupt Descriptor Table)
//!   - CPU utilities
//!   - PIC (Programmable Interrupt Controller)

pub mod gdt;
pub mod idt;
pub mod cpu;
pub mod pic;
