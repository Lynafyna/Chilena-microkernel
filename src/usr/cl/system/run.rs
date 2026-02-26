//! run — jalankan program CHN dari ChilenaFS

use crate::sys;
use crate::api::process::ExitCode;

pub fn run(args: &[&str]) {
    if args.is_empty() {
        println!("Usage: run <program.chn> [args...]");
        println!("  Contoh: run hello.chn");
        return;
    }

    let filename = args[0];
    let prog_args = if args.len() > 1 { &args[1..] } else { &[] };

    // Baca dari ChilenaFS dulu, fallback ke MemFS
    let bin = if sys::fs::chfs::is_mounted() {
        match sys::fs::chfs::read_file(filename) {
            Ok(data) => data,
            Err(_) => {
                // Coba MemFS
                match sys::fs::open_file(filename) {
                    Some(mut f) => {
                        use sys::fs::FileIO;
                        let mut buf = alloc::vec![0u8; f.size()];
                        match f.read(&mut buf) {
                            Ok(n) => { buf.truncate(n); buf }
                            Err(_) => {
                                println!("run: gagal baca '{}'", filename);
                                return;
                            }
                        }
                    }
                    None => {
                        println!("run: file '{}' tidak ditemukan", filename);
                        println!("  Gunakan chfs-write untuk upload program ke disk");
                        return;
                    }
                }
            }
        }
    } else {
        println!("run: ChilenaFS tidak mounted");
        return;
    };

    // Validasi magic CHN
    if bin.len() < 4 || &bin[0..4] != b"\x7fCHN" {
        println!("run: '{}' bukan file CHN yang valid", filename);
        println!("  File CHN harus dimulai dengan magic 0x7F 'C' 'H' 'N'");
        return;
    }

    println!("run: memuat '{}' ({} bytes)...", filename, bin.len());

    // Spawn proses — kernel perlu save context dulu
    // agar setelah proses exit, bisa kembali ke sini
    match sys::process::Process::spawn(&bin, prog_args.as_ptr() as usize, prog_args.len()) {
        Ok(_) => {
            // Proses selesai normal — shell lanjut
        }
        Err(ExitCode::ExecError) => {
            println!("run: gagal load '{}' — header CHN invalid atau memory tidak cukup", filename);
        }
        Err(e) => {
            println!("run: error ({:?})", e);
        }
    }
}
