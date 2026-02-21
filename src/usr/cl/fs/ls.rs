//! ls — list files in the filesystem

use crate::sys;

pub fn run(args: &[&str]) {
    let dir = args.first().copied().unwrap_or("/");
    let full_dir = match sys::fs::canonicalize(dir) {
        Ok(p) => p,
        Err(_) => { println!("ls: invalid path"); return; }
    };

    let files = sys::fs::list_files(&full_dir);

    if files.is_empty() {
        println!("(empty)");
    } else {
        for f in files.iter().filter(|f| !f.name.ends_with("/.dir")) {
            println!("  {:>8} B  {}", f.size, f.name);
        }
        let visible = files.iter().filter(|f| !f.name.ends_with("/.dir")).count();
        println!("--- {} file(s)", visible);
    }
}
