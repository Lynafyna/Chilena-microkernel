//! chfs-cat — baca isi file dari ChilenaFS

use crate::sys::fs::chfs;
use alloc::string::String;

pub fn run(args: &[&str]) {
    if args.is_empty() {
        println!("Usage: chfs-cat <nama_file>");
        return;
    }

    if !chfs::is_mounted() {
        println!("chfs-cat: ChilenaFS tidak ter-mount");
        return;
    }

    let filename = args[0];

    match chfs::read_file(filename) {
        Ok(data) => {
            let s = String::from_utf8_lossy(&data);
            print!("{}", s);
            if !s.ends_with('\n') { println!(); }
        }
        Err(e) => println!("chfs-cat: {} — '{}'", e, filename),
    }
}
