//! disk-ping — cek status VirtIO disk dan tampilkan info

use crate::sys::virtio;

pub fn run() {
    if !virtio::is_available() {
        println!("disk-ping: VirtIO tidak tersedia");
        println!("  Pastikan QEMU dijalankan dengan: make run-disk");
        return;
    }
    let cap = virtio::capacity();
    println!("VirtIO disk: OK");
    println!("  Kapasitas : {} sektor", cap);
    println!("  Ukuran    : {} MB", cap * 512 / 1_048_576);
    println!("  Sektor    : 512 bytes masing-masing");
    println!("");
    println!("Perintah disk yang tersedia:");
    println!("  disk-ping              -- info disk ini");
    println!("  disk-read <sector>     -- baca 512 bytes dari sektor");
}
