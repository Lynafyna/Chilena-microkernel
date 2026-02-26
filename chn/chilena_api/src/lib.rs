//! chilena_api — Library untuk program CHN
//!
//! Wrap semua syscall Chilena agar program bisa:
//!   - Print ke layar
//!   - Baca input
//!   - Kirim/terima IPC
//!   - Exit

#![no_std]
#![allow(dead_code)]

// ---------------------------------------------------------------------------
// Syscall numbers — harus sama persis dengan src/sys/syscall/number.rs
// ---------------------------------------------------------------------------
pub mod number {
    pub const EXIT:  usize = 0x01;
    pub const READ:  usize = 0x03;
    pub const WRITE: usize = 0x04;
    pub const OPEN:  usize = 0x05;
    pub const CLOSE: usize = 0x06;
    pub const SLEEP: usize = 0x0B;
    pub const SEND:  usize = 0x10;
    pub const RECV:  usize = 0x11;
}

// ---------------------------------------------------------------------------
// Raw syscall wrappers (x86_64 sysenter/syscall convention Chilena)
// Chilena pakai interrupt 0x80 (seperti Linux lama)
// ---------------------------------------------------------------------------

#[inline(always)]
pub unsafe fn syscall0(n: usize) -> usize {
    let ret: usize;
    core::arch::asm!(
        "int 0x80",
        in("rax") n,
        lateout("rax") ret,
        options(nostack)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall1(n: usize, a1: usize) -> usize {
    let ret: usize;
    core::arch::asm!(
        "int 0x80",
        in("rax") n,
        in("rdi") a1,
        lateout("rax") ret,
        options(nostack)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall2(n: usize, a1: usize, a2: usize) -> usize {
    let ret: usize;
    core::arch::asm!(
        "int 0x80",
        in("rax") n,
        in("rdi") a1,
        in("rsi") a2,
        lateout("rax") ret,
        options(nostack)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall3(n: usize, a1: usize, a2: usize, a3: usize) -> usize {
    let ret: usize;
    core::arch::asm!(
        "int 0x80",
        in("rax") n,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        lateout("rax") ret,
        options(nostack)
    );
    ret
}

#[inline(always)]
pub unsafe fn syscall4(n: usize, a1: usize, a2: usize, a3: usize, a4: usize) -> usize {
    let ret: usize;
    core::arch::asm!(
        "int 0x80",
        in("rax") n,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        lateout("rax") ret,
        options(nostack)
    );
    ret
}

// ---------------------------------------------------------------------------
// High-level API
// ---------------------------------------------------------------------------

/// Exit program dengan exit code
pub fn exit(code: usize) -> ! {
    unsafe { syscall1(number::EXIT, code); }
    loop {} // tidak pernah sampai sini
}

/// Tulis bytes ke stdout (handle 1)
pub fn write(buf: &[u8]) -> usize {
    unsafe {
        syscall3(number::WRITE, 1, buf.as_ptr() as usize, buf.len())
    }
}

/// Print string ke stdout
pub fn print(s: &str) {
    write(s.as_bytes());
}

/// Print string + newline
pub fn println(s: &str) {
    write(s.as_bytes());
    write(b"\n");
}

/// Sleep selama N detik
pub fn sleep(seconds: f64) {
    unsafe { syscall1(number::SLEEP, f64::to_bits(seconds) as usize); }
}

// ---------------------------------------------------------------------------
// Panic handler — wajib ada untuk no_std
// ---------------------------------------------------------------------------

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Kalau panic, print pesan dan exit
    write(b"[CHN PANIC]\n");
    exit(255)
}
