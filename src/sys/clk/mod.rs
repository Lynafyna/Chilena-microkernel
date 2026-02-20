//! Clock — Manajemen waktu Chilena
//!
//! Menyediakan:
//!   - uptime: waktu sejak boot (via PIT timer)
//!   - date: tanggal/waktu dari CMOS RTC
//!   - sleep: tunda eksekusi N detik

use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::instructions::{interrupts, port::Port};

// ---------------------------------------------------------------------------
// PIT Timer (IRQ 0) — ukur uptime dalam milidetik
// ---------------------------------------------------------------------------

/// Tick per detik (PIT dikonfigurasi ~1000 Hz)
const TICKS_PER_SEC: u64 = 1000;

static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    // Konfigurasi PIT channel 0, mode 3 (square wave), ~1000 Hz
    let divisor = 1193182u32 / TICKS_PER_SEC as u32;
    unsafe {
        let mut cmd: Port<u8>  = Port::new(0x43);
        let mut ch0: Port<u8>  = Port::new(0x40);
        cmd.write(0x36); // channel 0, lobyte/hibyte, mode 3
        ch0.write((divisor & 0xFF) as u8);
        ch0.write((divisor >> 8) as u8);
    }

    // Daftarkan handler IRQ 0 (timer)
    crate::sys::idt::set_irq_handler(0, on_tick);
}

fn on_tick() {
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Uptime kernel dalam detik (floating point)
pub fn uptime_secs() -> f64 {
    TICK_COUNT.load(Ordering::Relaxed) as f64 / TICKS_PER_SEC as f64
}

/// Tidur N detik (busy-wait via tick counter)
pub fn sleep(seconds: f64) {
    let target = TICK_COUNT.load(Ordering::Relaxed)
        + (seconds * TICKS_PER_SEC as f64) as u64;

    while TICK_COUNT.load(Ordering::Relaxed) < target {
        interrupts::enable_and_hlt();
    }
}

// ---------------------------------------------------------------------------
// RTC — Baca tanggal/waktu dari CMOS
// ---------------------------------------------------------------------------

fn cmos_read(reg: u8) -> u8 {
    unsafe {
        let mut addr: Port<u8> = Port::new(0x70);
        let mut data: Port<u8> = Port::new(0x71);
        addr.write(reg);
        data.read()
    }
}

fn bcd_to_bin(bcd: u8) -> u8 {
    (bcd & 0x0F) + ((bcd >> 4) * 10)
}

/// Baca tanggal dan waktu dari RTC CMOS
pub fn date_string() -> alloc::string::String {
    let sec  = bcd_to_bin(cmos_read(0x00));
    let min  = bcd_to_bin(cmos_read(0x02));
    let hour = bcd_to_bin(cmos_read(0x04));
    let day  = bcd_to_bin(cmos_read(0x07));
    let mon  = bcd_to_bin(cmos_read(0x08));
    let year = bcd_to_bin(cmos_read(0x09)) as u16 + 2000;

    alloc::format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year, mon, day, hour, min, sec)
}
