//! info — tampilkan informasi sistem Chilena

use crate::sys;

pub fn run() {
    println!("=== Chilena System Info ===");
    println!("Kernel  : Chilena v{}", crate::VERSION);
    println!("Uptime  : {:.3} detik", sys::clk::uptime_secs());
    println!("Tanggal : {}", sys::clk::date_string());
    println!("Memori  : {} MB total, {} MB bebas",
        sys::mem::total_memory() >> 20,
        sys::mem::free_memory()  >> 20,
    );
    println!("CWD     : {}", sys::process::cwd());
    if let Some(user) = sys::process::current_user() {
        println!("User    : {}", user);
    }
    println!("===========================");
}
