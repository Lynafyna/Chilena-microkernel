//! disk-write — tulis data ke sektor via VirtIO
//!
//! Saat ini mengakses VirtIO langsung karena shell masih jalan di kernel context.
//! TODO: Setelah userspace ELF loader siap, ini akan pakai IPC ke DiskServer.

use crate::sys::virtio;

pub fn run(args: &[&str]) {
    if args.len() < 2 {
        println!("Usage: disk-write <sector> <text>");
        println!("  Tulis text ke sektor (max 512 bytes)");
        return;
    }

    let sector: u64 = match args[0].parse() {
        Ok(n) => n,
        Err(_) => { println!("disk-write: sector harus angka"); return; }
    };

    if !virtio::is_available() {
        println!("disk-write: VirtIO tidak tersedia");
        return;
    }

    if sector >= virtio::capacity() {
        println!("disk-write: sektor {} di luar range", sector);
        return;
    }

    let text = args[1..].join(" ");
    let mut buf = [0u8; 512];
    let n = text.len().min(512);
    buf[..n].copy_from_slice(&text.as_bytes()[..n]);

    print!("Menulis ke sektor {}... ", sector);
    match virtio::write_sector(sector, &mut buf) {
        Ok(()) => println!("OK ({} bytes)", n),
        Err(e) => println!("GAGAL: {}", e),
    }
}
