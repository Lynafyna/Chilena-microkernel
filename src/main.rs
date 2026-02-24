#![no_std]
#![no_main]

extern crate alloc;

use bootloader::{entry_point, BootInfo};
use core::panic::PanicInfo;
use chilena::{sys, usr, hlt_loop};
use chilena::{kerror, kwarn, klog, print};

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    chilena::init(boot_info);
    print!("\x1b[?25h");
    boot_sequence();
    loop { x86_64::instructions::hlt(); }
}

fn boot_sequence() {
    if sys::virtio::is_available() {
        klog!("Disk: VirtIO ready");
    }

    let boot_script = "/ini/boot.sh";
    if sys::fs::exists(boot_script) {
        usr::cl::shell::run_script(boot_script).ok();
    }

    // Selalu jalankan interactive shell setelah boot script (atau kalau tidak ada)
    usr::cl::shell::run_interactive().ok();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(loc) = info.location() {
        kerror!("PANIC at {}:{}:{}", loc.file(), loc.line(), loc.column());
    } else {
        kerror!("PANIC: {}", info);
    }
    hlt_loop();
}
