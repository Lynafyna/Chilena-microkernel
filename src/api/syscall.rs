//! Syscall API Chilena — wrappers ergonomis untuk userspace

use crate::sys::syscall::number;
use crate::api::process::ExitCode;

pub fn exit(code: ExitCode) -> ! {
    unsafe { crate::sys::syscall::syscall1(number::EXIT, code as usize); }
    loop {}
}

pub fn sleep(seconds: f64) {
    unsafe { crate::sys::syscall::syscall1(number::SLEEP, f64::to_bits(seconds) as usize); }
}

pub fn open(path: &str, flags: u8) -> isize {
    unsafe {
        crate::sys::syscall::syscall3(
            number::OPEN,
            path.as_ptr() as usize,
            path.len(),
            flags as usize,
        ) as isize
    }
}

pub fn close(handle: usize) {
    unsafe { crate::sys::syscall::syscall1(number::CLOSE, handle); }
}

pub fn read(handle: usize, buf: &mut [u8]) -> isize {
    unsafe {
        crate::sys::syscall::syscall3(
            number::READ,
            handle,
            buf.as_mut_ptr() as usize,
            buf.len(),
        ) as isize
    }
}

pub fn write(handle: usize, buf: &[u8]) -> isize {
    unsafe {
        crate::sys::syscall::syscall3(
            number::WRITE,
            handle,
            buf.as_ptr() as usize,
            buf.len(),
        ) as isize
    }
}
