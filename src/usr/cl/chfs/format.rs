//! chfs-format — format disk dengan ChilenaFS

use crate::sys::fs::chfs;

pub fn run() {
    println!("WARNING: Ini akan menghapus SEMUA data di disk!");
    println!("Ketik 'yes' untuk lanjut:");

    let input = crate::sys::console::read_line();
    if input.trim() != "yes" {
        println!("Format dibatalkan.");
        return;
    }

    print!("Memformat disk dengan ChilenaFS... ");
    match chfs::format() {
        Ok(()) => {
            println!("SELESAI!");
            println!("Disk siap digunakan. Gunakan chfs-ls, chfs-write, chfs-cat.");
        }
        Err(e) => println!("GAGAL: {}", e),
    }
}
