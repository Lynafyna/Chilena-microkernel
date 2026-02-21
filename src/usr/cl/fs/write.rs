//! write — write text to a file

use crate::sys;

pub fn run(args: &[&str]) {
    if args.len() < 2 {
        println!("write: usage: write <file> <text>");
        return;
    }
    let path = args[0];
    let text = args[1..].join(" ");
    let full_path = match sys::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => { println!("write: invalid path"); return; }
    };
    let mut data = text.as_bytes().to_vec();
    data.push(b'\n');
    if sys::fs::write_file(&full_path, &data).is_ok() {
        println!("Written to '{}'", full_path);
    } else {
        println!("write: failed to write to '{}'", full_path);
    }
}
