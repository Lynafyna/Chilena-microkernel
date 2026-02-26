//! sysinfo.chn — Info sistem Chilena

#![no_std]
#![no_main]
use core::arch::global_asm;

global_asm!(r#"
.section .text
.global _start
_start:
    lea rsi, [rip + line1]
    mov rdx, 34
    call .print

    lea rsi, [rip + line2]
    mov rdx, 34
    call .print

    lea rsi, [rip + line3]
    mov rdx, 34
    call .print

    lea rsi, [rip + line4]
    mov rdx, 34
    call .print

    lea rsi, [rip + line5]
    mov rdx, 34
    call .print

    lea rsi, [rip + line6]
    mov rdx, 34
    call .print

    mov rax, 0x01
    xor rdi, rdi
    int 0x80
.hlt: hlt
    jmp .hlt

.print:
    mov rax, 0x04
    mov rdi, 1
    int 0x80
    ret

line1: .ascii "==================================\n"
line2: .ascii "   Chilena OS v0.1.0              \n"
line3: .ascii "   Arch    : x86_64               \n"
line4: .ascii "   Binary  : CHN format           \n"
line5: .ascii "   Status  : Userspace aktif!     \n"
line6: .ascii "==================================\n"
"#);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
