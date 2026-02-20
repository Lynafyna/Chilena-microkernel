//! PIC — Programmable Interrupt Controller (Intel 8259)
//!
//! Mengelola dua PIC yang dirangkai (master + slave) untuk
//! menangani 16 IRQ hardware eksternal.

use pic8259::ChainedPics;
use spin::Mutex;

/// Offset IRQ di IDT (IRQ 0-7 → vektor 32-39, IRQ 8-15 → vektor 40-47)
pub const PIC_MASTER_OFFSET: u8 = 32;
pub const PIC_SLAVE_OFFSET:  u8 = PIC_MASTER_OFFSET + 8;

/// Instance PIC global
pub static PICS: Mutex<ChainedPics> = Mutex::new(unsafe {
    ChainedPics::new(PIC_MASTER_OFFSET, PIC_SLAVE_OFFSET)
});

/// Inisialisasi PIC dan aktifkan interrupt CPU
pub fn init() {
    unsafe {
        PICS.lock().initialize();
    }
    x86_64::instructions::interrupts::enable();
}

/// Konversi nomor IRQ ke vektor IDT
pub fn irq_vector(irq: u8) -> u8 {
    PIC_MASTER_OFFSET + irq
}
