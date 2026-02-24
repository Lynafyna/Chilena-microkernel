//! chfs-ls — list semua file di ChilenaFS

use crate::sys::fs::chfs;

pub fn run() {
    if !chfs::is_mounted() {
        println!("chfs-ls: ChilenaFS tidak ter-mount");
        println!("  Jalankan 'chfs-format' untuk format disk dulu");
        return;
    }

    let files = chfs::list_all();
    if files.is_empty() {
        println!("(kosong — belum ada file di ChilenaFS)");
        return;
    }

    println!("ChilenaFS — daftar file:");
    println!("  {:<4}  {:<8}  {:<6}  {}", "ID", "SIZE", "TYPE", "NAME");
    println!("  {}", "-".repeat(40));

    for f in &files {
        let kind = if f.is_dir { "DIR" } else { "FILE" };
        println!("  {:<4}  {:<8}  {:<6}  {}",
            f.inode_id, f.size, kind, f.name);
    }

    let (count, _, next_sec) = chfs::info();
    println!("  {}", "-".repeat(40));
    println!("  Total: {} file/dir | Next free sector: {}", count, next_sec);
}
