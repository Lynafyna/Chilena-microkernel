//! chfs-rm — hapus file dari ChilenaFS

use crate::sys::fs::chfs;

pub fn run(args: &[&str]) {
    if args.is_empty() {
        println!("Usage: chfs-rm <nama_file>");
        return;
    }

    if !chfs::is_mounted() {
        println!("chfs-rm: ChilenaFS tidak ter-mount");
        return;
    }

    let filename = args[0];
    match chfs::remove(filename) {
        Ok(()) => println!("Dihapus: '{}'", filename),
        Err(e) => println!("GAGAL: {}", e),
    }
}
