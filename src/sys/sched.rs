//! Scheduler for Chilena — Round-Robin Preemptive (Proper Context Switch)
//!
//! FIXES:
//!   - TICK hanya di-increment sekali (tidak duplikat antara tick() & schedule())
//!   - schedule() adalah satu-satunya entry point scheduling
//!   - Proses baru tanpa saved stack_frame di-iretq langsung ke entry_point
//!   - switch_to() dihapus — redundant dan bisa race

use crate::sys::process::{
    CURRENT_PID, NEXT_PID, PROC_TABLE,
    save_registers, save_stack_frame,
    CpuRegisters,
};
use crate::sys::ipc::BlockState;
use crate::sys::gdt::GDT;

use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::registers::control::Cr3;
use x86_64::structures::idt::InterruptStackFrame;

// ---------------------------------------------------------------------------
// Scheduler interval
// ---------------------------------------------------------------------------

/// Switch process every 10ms (10 ticks @ 1000 Hz)
const SCHED_INTERVAL: u64 = 10;

static TICK: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// tick() — dipanggil dari clk::on_tick, HANYA increment counter
// Scheduling sesungguhnya ada di schedule() karena butuh akses ke stack frame
// ---------------------------------------------------------------------------

pub fn tick() {
    TICK.fetch_add(1, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// schedule() — dipanggil dari timer_handler di idt.rs
//              dengan frame & regs yang sudah di-save oleh naked function
// ---------------------------------------------------------------------------

pub fn schedule(
    frame: &mut InterruptStackFrame,
    regs:  &mut CpuRegisters,
) {
    let t = TICK.load(Ordering::Relaxed);
    if t % SCHED_INTERVAL != 0 {
        return;
    }

    let n_procs = NEXT_PID.load(Ordering::SeqCst);
    if n_procs <= 1 {
        return; // hanya ada kernel, tidak perlu switch
    }

    let cur = CURRENT_PID.load(Ordering::SeqCst);

    // Simpan state proses yang sedang jalan
    save_stack_frame(**frame);
    save_registers(*regs);

    // Cari proses berikutnya yang ready (skip PID 0 = kernel idle)
    let next = {
        let table = PROC_TABLE.read();
        let mut found = None;
        for i in 1..n_procs {
            let candidate = (cur + i) % n_procs;
            if candidate == 0 { continue; }
            if table[candidate].block == BlockState::Running {
                found = Some(candidate);
                break;
            }
        }
        found
    };

    let next_pid = match next {
        Some(p) if p != cur => p,
        _ => return, // tidak ada yang bisa jalan
    };

    // Ambil state proses berikutnya
    let (maybe_frame, next_regs, pt_frame, entry, stack) = {
        let table = PROC_TABLE.read();
        let p = &table[next_pid];
        (
            p.stack_frame,
            p.saved_regs,
            p.pt_frame,
            p.code_base + p.entry_point,
            p.stack_base,
        )
    };

    CURRENT_PID.store(next_pid, Ordering::SeqCst);

    // Restore register proses berikutnya
    *regs = next_regs;

    // Switch page table
    unsafe {
        let (_, flags) = Cr3::read();
        Cr3::write(pt_frame, flags);
    }

    if let Some(sf) = maybe_frame {
        // Proses sudah pernah jalan sebelumnya — restore saved frame-nya
        unsafe { frame.as_mut().write(sf); }
    } else {
        // Proses baru, belum pernah dijadwal — iretq ke entry point
        unsafe {
            core::arch::asm!(
                "cli",
                "push {ss:r}",
                "push {rsp:r}",
                "push 0x200",     // RFLAGS: IF=1
                "push {cs:r}",
                "push {rip:r}",
                "iretq",
                ss  = in(reg) GDT.1.u_data.0,
                rsp = in(reg) stack,
                cs  = in(reg) GDT.1.u_code.0,
                rip = in(reg) entry,
                options(noreturn)
            );
        }
    }
}
