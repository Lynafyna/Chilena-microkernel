//! cat — display file contents

use crate::sys;
use crate::sys::fs::FileIO;
use alloc::string::String;

pub fn run(args: &[&str]) {
    let path = match args.first() {
        Some(p) => p,
        None => { println!("cat: filename required"); return; }
    };

    let full_path = match sys::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => { println!("cat: invalid path"); return; }
    };

    if let Some(mut f) = sys::fs::open_file(&full_path) {
        let mut buf = alloc::vec![0u8; f.size().max(1)];
        if let Ok(n) = f.read(&mut buf) {
            let s = String::from_utf8_lossy(&buf[..n]);
            print!("{}", s);
            if !s.ends_with('\n') { println!(); }
        }
    } else {
        println!("cat: file '{}' not found", path);
    }
}
