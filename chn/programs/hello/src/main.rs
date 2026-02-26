//! hello.chn — Program CHN pertama untuk Chilena

#![no_std]
#![no_main]

use core::arch::global_asm;

global_asm!(r#"
.section .text
.global _start
_start:
    mov rax, 0x04
    mov rdi, 1
    lea rsi, [rip + msg]
    mov rdx, 16
    int 0x80

    mov rax, 0x01
    mov rdi, 0
    int 0x80

.loop:
    hlt
    jmp .loop

msg:
    .ascii "Halo dari CHN!\n\0"
"#);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
