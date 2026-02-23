//! disk-read — baca sektor dari disk via VirtIO
//!
//! Saat ini mengakses VirtIO langsung karena shell masih jalan di kernel context.
//! TODO: Setelah userspace ELF loader siap, ini akan pakai IPC ke DiskServer.

use crate::sys::virtio;

pub fn run(args: &[&str]) {
    if args.is_empty() {
        println!("Usage: disk-read <sector>");
        println!("  Baca 512 bytes dari sektor yang ditentukan");
        return;
    }

    let sector: u64 = match args[0].parse() {
        Ok(n) => n,
        Err(_) => { println!("disk-read: sector harus angka"); return; }
    };

    if !virtio::is_available() {
        println!("disk-read: VirtIO tidak tersedia");
        println!("  Jalankan QEMU dengan: make run-disk");
        return;
    }

    if sector >= virtio::capacity() {
        println!("disk-read: sektor {} di luar range (max {})",
            sector, virtio::capacity() - 1);
        return;
    }

    let mut buf = [0u8; 512];
    print!("Membaca sektor {}... ", sector);

    match virtio::read_sector(sector, &mut buf) {
        Ok(()) => {
            println!("OK");
            // Hex dump 64 bytes pertama
            println!("Hex dump (64 bytes pertama dari 512):");
            for row in 0..4 {
                let off = row * 16;
                print!("  {:04X}: ", off);
                for i in 0..16 { print!("{:02X} ", buf[off + i]); }
                print!(" |");
                for i in 0..16 {
                    let b = buf[off + i];
                    let c = if (0x20..0x7F).contains(&b) { b as char } else { '.' };
                    print!("{}", c);
                }
                println!("|");
            }
        }
        Err(e) => println!("GAGAL: {}", e),
    }
}
