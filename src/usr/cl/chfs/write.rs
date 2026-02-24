//! chfs-write — tulis file ke ChilenaFS

use crate::sys::fs::chfs;

pub fn run(args: &[&str]) {
    if args.len() < 2 {
        println!("Usage: chfs-write <nama_file> <isi>");
        println!("  Contoh: chfs-write hello.txt Halo dunia!");
        return;
    }

    if !chfs::is_mounted() {
        println!("chfs-write: ChilenaFS tidak ter-mount");
        println!("  Jalankan 'chfs-format' dulu");
        return;
    }

    let filename = args[0];
    let content  = args[1..].join(" ");

    match chfs::write_file(filename, content.as_bytes()) {
        Ok(()) => println!("Tersimpan: '{}' ({} bytes)", filename, content.len()),
        Err(e) => println!("GAGAL: {}", e),
    }
}
