//! counter.chn — Hitung 1 sampai 10

#![no_std]
#![no_main]
use core::arch::global_asm;

global_asm!(r#"
.section .text
.global _start
_start:
    mov rax, 0x04
    mov rdi, 1
    lea rsi, [rip + msg_hdr]
    mov rdx, 14
    int 0x80

    mov r12, 1          // counter = 1

.loop:
    cmp r12, 11
    jge .done

    cmp r12, 10
    je .print_ten

    // Single digit
    mov rax, r12
    add rax, 48
    lea rdi, [rip + buf]
    mov [rdi], al
    mov byte ptr [rdi+1], 10
    mov rax, 0x04
    mov rdi, 1
    lea rsi, [rip + buf]
    mov rdx, 2
    int 0x80
    jmp .next

.print_ten:
    mov rax, 0x04
    mov rdi, 1
    lea rsi, [rip + ten]
    mov rdx, 3
    int 0x80

.next:
    inc r12
    jmp .loop

.done:
    mov rax, 0x04
    mov rdi, 1
    lea rsi, [rip + msg_done]
    mov rdx, 9
    int 0x80

    mov rax, 0x01
    xor rdi, rdi
    int 0x80
.hlt: hlt
    jmp .hlt

msg_hdr:  .ascii "Menghitung:\n\0\0"
msg_done: .ascii "Selesai!\n"
ten:      .ascii "10\n"
buf:      .byte 0, 0, 0, 0
"#);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
