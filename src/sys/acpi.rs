//! ACPI — Power management (shutdown/reboot)
//!
//! Implementasi minimal: hanya mendukung power off via ACPI PM1a.

use x86_64::instructions::port::Port;

static mut PM1A_CNT: u32 = 0;
static mut SLP_TYPA: u16 = 0;
#[allow(dead_code)]
const  SLP_EN:       u16 = 1 << 13;

pub fn init() {
    // Pada emulator QEMU, power off bisa dilakukan via port 0x604
    // Untuk hardware nyata dibutuhkan parsing ACPI tables
    // (implementasi lanjutan bisa menggunakan crate `acpi`)
    klog!("ACPI: init (minimal mode)");

    // QEMU power off magic
    unsafe { PM1A_CNT = 0x604; SLP_TYPA = 0; }
}

/// Matikan sistem
pub fn power_off() -> ! {
    klog!("ACPI: power off...");
    unsafe {
        // QEMU: tulis ke port 0x604
        let mut port: Port<u16> = Port::new(0x604);
        port.write(0x2000);

        // Fallback: halt loop
        loop { x86_64::instructions::hlt(); }
    }
}
