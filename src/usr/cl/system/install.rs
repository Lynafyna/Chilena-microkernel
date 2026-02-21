//! install — setup initial filesystem

use crate::sys;

pub fn run() {
    if sys::fs::is_mounted() {
        println!("Chilena is already installed!");
        return;
    }
    println!("Installing Chilena...");
    sys::fs::mount_memfs();
    sys::fs::mkdir("/ini");
    sys::fs::write_file("/ini/boot.sh", b"shell\n").ok();
    sys::fs::write_file("/ini/readme.txt", b"Welcome to Chilena!\n").ok();
    println!("Installation complete! Type \'reboot\' to restart.");
}
