//! mkdir — create a directory

use crate::sys;

pub fn run(args: &[&str]) {
    let path = match args.first() {
        Some(p) => p,
        None => { println!("mkdir: directory name required"); return; }
    };
    let full_path = match sys::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => { println!("mkdir: invalid path"); return; }
    };
    if sys::fs::dir_exists(&full_path) {
        println!("mkdir: '{}' already exists", full_path);
        return;
    }
    sys::fs::mkdir(&full_path);
    println!("Directory '{}' created", full_path);
}
