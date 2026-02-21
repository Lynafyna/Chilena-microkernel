//! cd — change working directory

use crate::sys;

pub fn run(args: &[&str]) {
    let path = args.first().copied().unwrap_or("/");
    let full_path = match sys::fs::canonicalize(path) {
        Ok(p) => p,
        Err(_) => { println!("cd: invalid path"); return; }
    };
    if full_path != "/" && !sys::fs::dir_exists(&full_path) {
        println!("cd: directory '{}' not found", path);
        return;
    }
    sys::process::set_cwd(&full_path);
}
