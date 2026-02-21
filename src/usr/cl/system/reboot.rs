//! reboot — restart the system

pub fn run() {
    println!("Rebooting...");
    unsafe { crate::sys::syscall::syscall1(crate::sys::syscall::number::HALT, 0xCAFE); }
}
