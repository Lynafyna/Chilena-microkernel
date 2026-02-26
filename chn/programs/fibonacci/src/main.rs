//! fibonacci.chn — Deret Fibonacci 10 angka

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
    mov rdx, 18
    int 0x80

    xor r12, r12        // a = 0
    mov r13, 1          // b = 1
    mov r14, 10         // count = 10

.loop:
    cmp r14, 0
    je .done

    // Print r12
    mov rdi, r12
    call .print_num

    mov rax, 0x04
    mov rdi, 1
    lea rsi, [rip + nl]
    mov rdx, 1
    int 0x80

    // next fib
    mov rax, r12
    add rax, r13
    mov r12, r13
    mov r13, rax

    dec r14
    jmp .loop

.done:
    mov rax, 0x01
    xor rdi, rdi
    int 0x80
.hlt: hlt
    jmp .hlt

// print_num: rdi = angka
.print_num:
    push rbx
    push r15
    lea rbx, [rip + numbuf]
    add rbx, 19
    mov byte ptr [rbx], 0
    mov r15, rbx

    mov rax, rdi
    test rax, rax
    jnz .digits
    dec rbx
    mov byte ptr [rbx], 48
    jmp .write

.digits:
    test rax, rax
    jz .write
    xor rdx, rdx
    mov rcx, 10
    div rcx
    add dl, 48
    dec rbx
    mov [rbx], dl
    jmp .digits

.write:
    mov rdx, r15
    sub rdx, rbx
    mov rax, 0x04
    mov rdi, 1
    mov rsi, rbx
    int 0x80
    pop r15
    pop rbx
    ret

msg_hdr: .ascii "Deret Fibonacci:\n\0"
nl:      .ascii "\n"
numbuf:  .space 21
"#);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! { loop {} }
