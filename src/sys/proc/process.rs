//! Process Manager for Chilena
//!
//! Manages the process table, I/O handles, context switching,
//! and loading CHN binaries into userspace memory.
//!
//! Format binary yang didukung: CHN (Chilena Native) — format custom Chilena.
//! ELF tidak didukung — Chilena punya format sendiri.

use crate::api::process::ExitCode;
use crate::sys;
use crate::sys::console::Console;
use crate::sys::fs::{Resource, Device};
use crate::sys::gdt::GDT;
use crate::sys::ipc::{BlockState, Message};
use crate::sys::mem::{phys_mem_offset, with_frame_allocator};

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use lazy_static::lazy_static;
use linked_list_allocator::LockedHeap;
use spin::RwLock;
use x86_64::registers::control::Cr3;
use x86_64::structures::idt::InterruptStackFrameValue;
use x86_64::structures::paging::{
    FrameAllocator, FrameDeallocator, OffsetPageTable, PageTable,
    PageTableFlags, PhysFrame, Translate, mapper::TranslateResult,
};
use x86_64::VirtAddr;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// CHN header magic: 0x7F 'C' 'H' 'N'
pub const CHN_MAGIC: [u8; 4] = [0x7F, b'C', b'H', b'N'];

/// Ukuran CHN header dalam bytes
pub const CHN_HEADER_SIZE: usize = 32;

pub const MAX_HANDLES:  usize = 64;
pub const MAX_PROCS:    usize = 8;
pub const MAX_PROC_MEM: usize = 10 << 20; // 10 MB per process

/// Start address of userspace
const USER_BASE: u64 = 0x0080_0000;

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------

/// Monotonic counter — hanya dipakai sebagai fallback
static PROC_CODE_BASE: AtomicU64    = AtomicU64::new(0);
pub static CURRENT_PID: AtomicUsize = AtomicUsize::new(0);
pub static NEXT_PID:    AtomicUsize = AtomicUsize::new(1);
/// Jumlah proses aktif (tidak termasuk PID 0 / kernel idle).
/// Ini terpisah dari NEXT_PID yang merupakan counter monotonik.
pub static ACTIVE_PROCS: AtomicUsize = AtomicUsize::new(0);

/// Kernel RSP yang disimpan sebelum jump ke proses CHN
/// Dipakai untuk kembali ke kernel setelah proses exit
static KERNEL_RSP: AtomicU64 = AtomicU64::new(0);
static KERNEL_RSP2: AtomicU64 = AtomicU64::new(0);  // kernel RSP setelah return

pub fn get_kernel_rsp() -> u64  { KERNEL_RSP.load(Ordering::SeqCst) }
pub fn get_kernel_rsp2() -> u64 { KERNEL_RSP2.load(Ordering::SeqCst) }

lazy_static! {
    pub static ref PROC_TABLE: RwLock<[Box<Process>; MAX_PROCS]> = {
        RwLock::new([(); MAX_PROCS].map(|_| Box::new(Process::new())))
    };
}

pub fn set_proc_code_base(addr: u64) {
    PROC_CODE_BASE.store(addr, Ordering::SeqCst);
}

/// Cari slot kosong di process table (PID > 0)
/// FIX: slot reuse — slot yang id==0 dan bukan PID 0 berarti free
fn find_free_slot() -> Option<usize> {
    let table = PROC_TABLE.read();
    for i in 1..MAX_PROCS {
        if table[i].id == 0 {
            return Some(i);
        }
    }
    None
}

/// Cari virtual address range yang belum dipakai proses manapun
/// FIX: virtual address reuse — daripada terus nambah PROC_CODE_BASE
fn find_free_code_base() -> Option<u64> {
    let slot_size = MAX_PROC_MEM as u64;
    let max_slots = (MAX_PROCS - 1) as u64;
    let table = PROC_TABLE.read();

    'outer: for slot in 0..max_slots {
        let candidate = USER_BASE + slot * slot_size;
        for i in 1..MAX_PROCS {
            if table[i].id != 0 && table[i].code_base == candidate {
                continue 'outer;
            }
        }
        return Some(candidate);
    }
    None
}

// ---------------------------------------------------------------------------
// CHN Binary Format — Chilena Native Executable
//
// Header 32 bytes:
//   [0..4]   magic        = 0x7F 'C' 'H' 'N'
//   [4..6]   version      = 1
//   [6..8]   flags        = 0x0001 executable | 0x0002 debug
//   [8..12]  entry_offset = offset dari awal CODE ke entry point
//   [12..16] code_size    = ukuran section kode (bytes)
//   [16..20] data_size    = ukuran section data (bytes)
//   [20..24] stack_size   = stack yang diminta (default 65536)
//   [24..28] min_memory   = minimum RAM yang dibutuhkan
//   [28..30] target_arch  = 0x01 = x86_64
//   [30]     os_version   = minimum Chilena version
//   [31]     checksum     = XOR semua 31 bytes sebelumnya
//
// Setelah header:
//   [32 .. 32+code_size]            : kode program
//   [32+code_size .. 32+code_size+data_size] : data (string, konstanta)
// ---------------------------------------------------------------------------

pub struct ChnHeader {
    pub version:      u16,
    pub flags:        u16,
    pub entry_offset: u32,
    pub code_size:    u32,
    pub data_size:    u32,
    pub stack_size:   u32,
    pub min_memory:   u32,
    pub target_arch:  u16,
    pub os_version:   u8,
    pub checksum:     u8,
}

impl ChnHeader {
    /// Parse CHN header dari bytes — validasi magic + checksum
    pub fn parse(bin: &[u8]) -> Option<Self> {
        if bin.len() < CHN_HEADER_SIZE { return None; }

        // Cek magic
        if &bin[0..4] != &CHN_MAGIC { return None; }

        // Cek checksum — XOR semua 31 bytes pertama
        let expected_checksum = bin[..31].iter().fold(0u8, |acc, &b| acc ^ b);
        if expected_checksum != bin[31] {
            kwarn!("CHN: checksum mismatch (got {:#X}, expected {:#X})",
                bin[31], expected_checksum);
            return None;
        }

        let version      = u16::from_le_bytes(bin[4..6].try_into().ok()?);
        let flags        = u16::from_le_bytes(bin[6..8].try_into().ok()?);
        let entry_offset = u32::from_le_bytes(bin[8..12].try_into().ok()?);
        let code_size    = u32::from_le_bytes(bin[12..16].try_into().ok()?);
        let data_size    = u32::from_le_bytes(bin[16..20].try_into().ok()?);
        let stack_size   = u32::from_le_bytes(bin[20..24].try_into().ok()?);
        let min_memory   = u32::from_le_bytes(bin[24..28].try_into().ok()?);
        let target_arch  = u16::from_le_bytes(bin[28..30].try_into().ok()?);
        let os_version   = bin[30];
        let checksum     = bin[31];

        // Validasi arch — hanya x86_64 (0x01)
        if target_arch != 0x01 {
            kwarn!("CHN: unsupported arch {:#X}", target_arch);
            return None;
        }

        // Validasi ukuran
        let total_expected = CHN_HEADER_SIZE + code_size as usize + data_size as usize;
        if bin.len() < total_expected {
            kwarn!("CHN: binary too small ({} < {})", bin.len(), total_expected);
            return None;
        }

        Some(Self { version, flags, entry_offset, code_size, data_size,
                    stack_size, min_memory, target_arch, os_version, checksum })
    }
}

#[repr(align(8), C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CpuRegisters {
    // Callee-saved (System V ABI) — harus disimpan saat context switch
    pub r15: usize,
    pub r14: usize,
    pub r13: usize,
    pub r12: usize,
    pub rbp: usize,
    pub rbx: usize,
    // Caller-saved (scratch) — sudah ada sebelumnya
    pub r11: usize,
    pub r10: usize,
    pub r9:  usize,
    pub r8:  usize,
    pub rdi: usize,
    pub rsi: usize,
    pub rdx: usize,
    pub rcx: usize,
    pub rax: usize,
}

// ---------------------------------------------------------------------------
// Process data (env, cwd, handles)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct ProcData {
    pub env:     BTreeMap<String, String>,
    pub cwd:     String,
    pub user:    Option<String>,
    pub handles: [Option<Box<Resource>>; MAX_HANDLES],
}

impl ProcData {
    pub fn new(cwd: &str, user: Option<&str>) -> Self {
        let mut handles = [(); MAX_HANDLES].map(|_| None);

        // stdin=0, stdout=1, stderr=2, null=3
        handles[0] = Some(Box::new(Resource::Device(Device::Console(Console::new()))));
        handles[1] = Some(Box::new(Resource::Device(Device::Console(Console::new()))));
        handles[2] = Some(Box::new(Resource::Device(Device::Console(Console::new()))));
        handles[3] = Some(Box::new(Resource::Device(Device::Null)));

        Self {
            env:  BTreeMap::new(),
            cwd:  cwd.to_string(),
            user: user.map(String::from),
            handles,
        }
    }
}

// ---------------------------------------------------------------------------
// Process API — access current process state
// ---------------------------------------------------------------------------

pub fn current_pid() -> usize       { CURRENT_PID.load(Ordering::SeqCst) }
pub fn set_pid(id: usize)           { CURRENT_PID.store(id, Ordering::SeqCst); }

pub fn cwd() -> String {
    PROC_TABLE.read()[current_pid()].data.cwd.clone()
}

pub fn set_cwd(path: &str) {
    PROC_TABLE.write()[current_pid()].data.cwd = path.to_string();
}

pub fn env_var(key: &str) -> Option<String> {
    PROC_TABLE.read()[current_pid()].data.env.get(key).cloned()
}

pub fn set_env_var(key: &str, val: &str) {
    PROC_TABLE.write()[current_pid()].data.env.insert(key.into(), val.into());
}

pub fn current_user() -> Option<String> {
    PROC_TABLE.read()[current_pid()].data.user.clone()
}

// ---------------------------------------------------------------------------
// Handle management
// ---------------------------------------------------------------------------

pub fn alloc_handle(res: Resource) -> Result<usize, ()> {
    let mut table = PROC_TABLE.write();
    let proc = &mut table[current_pid()];
    for i in 4..MAX_HANDLES {
        if proc.data.handles[i].is_none() {
            proc.data.handles[i] = Some(Box::new(res));
            return Ok(i);
        }
    }
    Err(())
}

pub fn get_handle(h: usize) -> Option<Box<Resource>> {
    PROC_TABLE.read()[current_pid()].data.handles[h].clone()
}

pub fn update_handle(h: usize, res: Resource) {
    PROC_TABLE.write()[current_pid()].data.handles[h] = Some(Box::new(res));
}

pub fn free_handle(h: usize) {
    PROC_TABLE.write()[current_pid()].data.handles[h] = None;
}

// ---------------------------------------------------------------------------
// Saved registers & stack frame (for spawn/exit context switch)
// ---------------------------------------------------------------------------

pub fn saved_registers() -> CpuRegisters {
    PROC_TABLE.read()[current_pid()].saved_regs
}

pub fn save_registers(r: CpuRegisters) {
    PROC_TABLE.write()[current_pid()].saved_regs = r;
}

pub fn saved_stack_frame() -> Option<InterruptStackFrameValue> {
    PROC_TABLE.read()[current_pid()].stack_frame
}

pub fn save_stack_frame(sf: InterruptStackFrameValue) {
    PROC_TABLE.write()[current_pid()].stack_frame = Some(sf);
}

// ---------------------------------------------------------------------------
// Memory address helpers
// ---------------------------------------------------------------------------

pub fn code_base() -> u64 {
    PROC_TABLE.read()[current_pid()].code_base
}

/// Convert a userspace pointer (possibly relative) to an absolute kernel address
pub fn resolve_addr(addr: u64) -> *mut u8 {
    let base = code_base();
    if addr < base { (base + addr) as *mut u8 } else { addr as *mut u8 }
}

pub fn is_user_addr(addr: u64) -> bool {
    USER_BASE <= addr && addr <= USER_BASE + MAX_PROC_MEM as u64
}

// ---------------------------------------------------------------------------
// Per-process memory allocation
// ---------------------------------------------------------------------------

pub unsafe fn user_alloc(layout: Layout) -> *mut u8 {
    PROC_TABLE.read()[current_pid()].allocator.alloc(layout)
}

pub unsafe fn user_free(ptr: *mut u8, layout: Layout) {
    let table = PROC_TABLE.read();
    let proc  = &table[current_pid()];
    let bot   = proc.allocator.lock().bottom();
    let top   = proc.allocator.lock().top();
    if (bot as u64) <= ptr as u64 && ptr < top {
        proc.allocator.dealloc(ptr, layout);
    }
}

// ---------------------------------------------------------------------------
// Per-process page table
// ---------------------------------------------------------------------------

unsafe fn current_page_table_frame() -> PhysFrame {
    PROC_TABLE.read()[current_pid()].pt_frame
}

pub unsafe fn page_table() -> &'static mut PageTable {
    sys::mem::create_page_table_from_frame(current_page_table_frame())
}

// ---------------------------------------------------------------------------
// Process termination
// ---------------------------------------------------------------------------

pub fn terminate() {
    let pid = current_pid();

    // FIX BUG #4: Ambil SEMUA data yang dibutuhkan dalam satu lock,
    // lalu lepas lock sebelum memanggil release_pages().
    // Sebelumnya release_pages() dipanggil saat lock masih dipegang,
    // dan clean_up() di dalam unmap_page bisa trigger page fault
    // yang butuh PROC_TABLE.read() lagi → deadlock.
    let (parent_id, pt_frame, code_base, stack_base) = {
        let table = PROC_TABLE.read();
        let proc  = &table[pid];
        (proc.parent_id, proc.pt_frame, proc.code_base, proc.stack_base)
    };
    // Lock sudah dilepas di sini — aman untuk operasi yang bisa trigger page fault

    // Release halaman proses TANPA memegang lock PROC_TABLE
    release_process_pages(pt_frame, code_base, stack_base);

    // Clear slot — set id=0 menandakan slot kosong dan siap di-reuse
    {
        let mut table = PROC_TABLE.write();
        table[pid] = Box::new(Process::new());
    }

    // Update jumlah proses aktif
    ACTIVE_PROCS.fetch_sub(1, Ordering::SeqCst);

    set_pid(parent_id);

    // Deallocate page table frame dan switch ke page table parent
    unsafe {
        let (_, flags) = Cr3::read();
        with_frame_allocator(|fa| {
            fa.deallocate_frame(pt_frame);
        });
        // Ambil parent_pt dalam lock singkat yang tidak bisa deadlock
        // (tidak ada operasi memory di dalamnya)
        let parent_pt = PROC_TABLE.read()[parent_id].pt_frame;
        Cr3::write(parent_pt, flags);
    }
}

/// Bebaskan semua halaman milik proses tanpa memegang lock PROC_TABLE.
/// Fungsi ini menerima data mentah sehingga tidak perlu akses tabel proses.
fn release_process_pages(pt_frame: PhysFrame, code_base: u64, _stack_base: u64) {
    let pt     = unsafe { sys::mem::create_page_table_from_frame(pt_frame) };
    let mut mapper = unsafe {
        OffsetPageTable::new(pt, VirtAddr::new(phys_mem_offset()))
    };
    sys::mem::unmap_page(&mut mapper, code_base, MAX_PROC_MEM);

    // Juga cek apakah ada mapping di USER_BASE yang perlu dibersihkan
    match mapper.translate(VirtAddr::new(USER_BASE)) {
        TranslateResult::Mapped { flags, .. } => {
            if flags.contains(PageTableFlags::USER_ACCESSIBLE) {
                sys::mem::unmap_page(&mut mapper, USER_BASE, MAX_PROC_MEM);
            }
        }
        _ => {}
    }
}

pub fn power_off_hook() {
    terminate();
    sys::acpi::power_off();
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// KernelContext — simpan full register state untuk longjmp balik ke kernel
// ---------------------------------------------------------------------------
#[repr(C)]
pub struct KernelContext {
    pub rsp: u64,
    pub r15: u64, pub r14: u64, pub r13: u64, pub r12: u64,
    pub rbp: u64, pub rbx: u64,
    pub rip: u64,
}

static mut KERNEL_CTX: KernelContext = KernelContext {
    rsp: 0, r15: 0, r14: 0, r13: 0, r12: 0,
    rbp: 0, rbx: 0, rip: 0,
};

#[unsafe(naked)]
unsafe extern "sysv64" fn spawn_exec_save_rsp(
    proc:     *const Process,
    args_ptr: usize,
    args_len: usize,
) {
    core::arch::naked_asm!(
        // Simpan semua callee-saved + RSP + return address ke KERNEL_CTX
        // Layout: rsp=0, r15=8, r14=16, r13=24, r12=32, rbp=40, rbx=48, rip=56
        "mov [{ctx} + 0],  rsp",
        "mov [{ctx} + 8],  r15",
        "mov [{ctx} + 16], r14",
        "mov [{ctx} + 24], r13",
        "mov [{ctx} + 32], r12",
        "mov [{ctx} + 40], rbp",
        "mov [{ctx} + 48], rbx",
        "mov rax, [rsp]",            // return address dari stack
        "mov [{ctx} + 56], rax",
        // Panggil exec_raw(proc, args_ptr, args_len)
        "call {exec}",
        ctx  = sym KERNEL_CTX,
        exec = sym Process::exec_raw,
    );
}

/// Dipanggil oleh IDT EXIT handler — restore kernel context dan return
pub unsafe fn kernel_longjmp() -> ! {
    core::arch::asm!(
        "mov r15, [{ctx} + 8]",
        "mov r14, [{ctx} + 16]",
        "mov r13, [{ctx} + 24]",
        "mov r12, [{ctx} + 32]",
        "mov rbp, [{ctx} + 40]",
        "mov rbx, [{ctx} + 48]",
        "mov rcx, [{ctx} + 56]",   // return address
        "mov rsp, [{ctx} + 0]",    // restore RSP
        "add rsp, 8",              // pop return address (sudah di rcx)
        "mov ax, 0x20",            // restore kernel DS
        "mov ds, ax",
        "mov es, ax",
        "jmp rcx",                 // jump ke return address
        ctx = sym KERNEL_CTX,
        options(noreturn)
    );
}
//
// Menggunakan naked function agar kontrol stack 100% manual.
// Argumen (System V AMD64 ABI):
//   rdi = entry (RIP tujuan)
//   rsi = stack_top (RSP userspace)
//   rdx = u_code (CS selector)
//   rcx = u_data (SS + DS/ES/FS/GS selector)
//   r8  = args_ptr (untuk rdi userspace)
//   r9  = args_len (untuk rsi userspace)
// ---------------------------------------------------------------------------
#[unsafe(naked)]
unsafe extern "sysv64" fn jump_to_userspace(
    entry:     u64,  // rdi
    stack_top: u64,  // rsi
    u_code:    u64,  // rdx
    u_data:    u64,  // rcx
    args_ptr:  u64,  // r8
    args_len:  u64,  // r9
) -> ! {
    core::arch::naked_asm!(
        // Set data segment registers ke u_data (rcx)
        "mov ax, cx",
        "mov ds, ax",
        "mov es, ax",
        // Build iretq frame
        "push rcx",       // SS
        "push rsi",       // RSP userspace
        "push 0x202",     // RFLAGS
        "push rdx",       // CS
        "push rdi",       // RIP = entry
        // Set argumen userspace
        "mov rdi, r8",
        "mov rsi, r9",
        "xor rax, rax",
        "xor rbx, rbx",
        "xor rcx, rcx",
        "xor rdx, rdx",
        "xor r8,  r8",
        "xor r9,  r9",
        "xor r10, r10",
        "xor r11, r11",
        "iretq",
    );
}

// ---------------------------------------------------------------------------
// Process struct
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Process {
    pub id:          usize,
    pub parent_id:   usize,
    pub code_base:   u64,
    pub stack_base:  u64,
    pub entry_point: u64,
    pub pt_frame:    PhysFrame,
    pub stack_frame: Option<InterruptStackFrameValue>,
    pub saved_regs:  CpuRegisters,
    pub data:        ProcData,
    pub allocator:   Arc<LockedHeap>,
    /// IPC mailbox — single incoming message slot
    pub mailbox:     Option<Message>,
    /// Process block state (Running / WaitingSend / WaitingRecv)
    pub block:       BlockState,
}

impl Process {
    pub fn new() -> Self {
        Self {
            id:          0,
            parent_id:   0,
            code_base:   0,
            stack_base:  0,
            entry_point: 0,
            pt_frame:    Cr3::read().0,
            stack_frame: None,
            saved_regs:  CpuRegisters::default(),
            data:        ProcData::new("/", None),
            allocator:   Arc::new(LockedHeap::empty()),
            mailbox:     None,
            block:       BlockState::Running,
        }
    }

    pub fn spawn(bin: &[u8], args_ptr: usize, args_len: usize) -> Result<(), ExitCode> {
        if let Ok(id) = Self::create(bin) {
            let proc = PROC_TABLE.read()[id].clone();
            unsafe { spawn_exec_save_rsp(&*proc, args_ptr, args_len); }
            // Kembali ke sini setelah kernel_longjmp dari EXIT syscall
            // Ini berarti proses selesai dengan sukses
            return Ok(());
        }
        Err(ExitCode::ExecError)
    }

    fn create(bin: &[u8]) -> Result<usize, ()> {
        // FIX: cari slot kosong, bukan check NEXT_PID >= MAX_PROCS
        let slot = find_free_slot().ok_or(())?;

        // FIX: cari virtual address range yang bisa di-reuse
        let code_base = find_free_code_base().ok_or(())?;

        // Allocate frame for new process page table
        let pt_frame = with_frame_allocator(|fa| {
            fa.allocate_frame().expect("could not allocate frame for page table")
        });

        let new_pt     = unsafe { sys::mem::create_page_table_from_frame(pt_frame) };
        let kernel_pt  = unsafe { sys::mem::active_page_table() };

        // Copy entire kernel page table to new process
        for (dst, src) in new_pt.iter_mut().zip(kernel_pt.iter()) {
            *dst = src.clone();
        }

        let mut mapper = unsafe {
            OffsetPageTable::new(new_pt, VirtAddr::new(phys_mem_offset()))
        };

        let stack_base = code_base + MAX_PROC_MEM as u64 - 4096;
        let mut entry_point = 0u64;

        // Parse dan load CHN binary
        let hdr = ChnHeader::parse(bin).ok_or(())?;

        // Validasi memory requirement
        let stack_sz = if hdr.stack_size == 0 { 65536 } else { hdr.stack_size as usize };
        let total_needed = hdr.code_size as usize + hdr.data_size as usize + stack_sz;
        if total_needed > MAX_PROC_MEM {
            kwarn!("CHN: program butuh {} bytes, max {}", total_needed, MAX_PROC_MEM);
            return Err(());
        }

        // Load code section ke code_base
        let code_start = CHN_HEADER_SIZE;
        let code_end   = code_start + hdr.code_size as usize;
        Self::load_segment(&mut mapper, code_base,
            hdr.code_size as usize, &bin[code_start..code_end])?;

        // Load data section setelah code (jika ada)
        if hdr.data_size > 0 {
            let data_start  = code_end;
            let data_end    = data_start + hdr.data_size as usize;
            let data_vaddr  = code_base + hdr.code_size as u64;
            Self::load_segment(&mut mapper, data_vaddr,
                hdr.data_size as usize, &bin[data_start..data_end])?;
        }

        // Entry point = code_base + entry_offset dari header
        entry_point = hdr.entry_offset as u64;

        klog!("CHN: loaded code={}B data={}B stack={}B entry=+{:#X}",
            hdr.code_size, hdr.data_size, stack_sz, entry_point);

        // Map stack pages — WAJIB sebelum proses jalan!
        // stack_base adalah puncak stack (RSP awal), tumbuh ke bawah
        // Kita map stack_sz bytes di bawah stack_base
        let stack_start = stack_base - stack_sz as u64;
        let empty_stack = alloc::vec![0u8; stack_sz];
        Self::load_segment(&mut mapper, stack_start, stack_sz, &empty_stack)?;
        klog!("CHN: stack mapped @ {:#X} - {:#X}", stack_start, stack_base);

        let parent = PROC_TABLE.read()[current_pid()].clone();

        let proc = Process {
            id:          slot, // gunakan slot index sebagai PID
            parent_id:   parent.id,
            code_base,
            stack_base,
            entry_point,
            pt_frame,
            data:        parent.data.clone(),
            stack_frame: None, // proses baru — belum punya saved frame
            saved_regs:  CpuRegisters::default(),
            allocator:   Arc::new(LockedHeap::empty()),
            mailbox:     None,
            block:       BlockState::Running,
        };

        PROC_TABLE.write()[slot] = Box::new(proc);
        NEXT_PID.fetch_add(1, Ordering::SeqCst);
        ACTIVE_PROCS.fetch_add(1, Ordering::SeqCst);
        Ok(slot)
    }

    fn exec(&self, args_ptr: usize, args_len: usize) {
        self.exec_raw(args_ptr, args_len)
    }

    fn exec_raw(&self, args_ptr: usize, args_len: usize) {
        let pt  = unsafe { page_table() };
        let mut mapper = unsafe {
            OffsetPageTable::new(pt, VirtAddr::new(phys_mem_offset()))
        };

        let args: &[&str] = unsafe {
            let ptr = resolve_addr(args_ptr as u64) as usize;
            core::slice::from_raw_parts(ptr as *const &str, args_len)
        };

        // FIX BUG #5: Hitung total ukuran yang dibutuhkan dulu sebelum map.
        // Sebelumnya hanya map 1 byte (= 1 page = 4096 bytes) yang bisa overflow
        // jika total panjang argumen + slice metadata > 4096 bytes.
        let total_str_bytes: usize = args.iter().map(|a| a.len()).sum();
        let align = core::mem::align_of::<&str>();
        // str data + alignment padding + slice of &str (16 bytes per entry di x86_64)
        let slice_meta_bytes = args_len * core::mem::size_of::<&str>();
        let needed = total_str_bytes + align + slice_meta_bytes + align;
        // Bulatkan ke atas ke kelipatan 4096, minimal 1 page
        let pages_needed = (needed + 4095) / 4096;
        let map_size = pages_needed * 4096;

        let args_base = self.code_base + (self.stack_base - self.code_base) / 2;
        sys::mem::map_page(&mut mapper, args_base, map_size).expect("args alloc");

        let mut cursor = args_base;
        let mut str_slices = alloc::vec::Vec::new();

        for arg in args {
            let dst = cursor as *mut u8;
            cursor += arg.len() as u64;
            unsafe {
                let s = core::slice::from_raw_parts_mut(dst, arg.len());
                s.copy_from_slice(arg.as_bytes());
                str_slices.push(core::str::from_utf8_unchecked(s));
            }
        }

        // Align ke pointer size
        let align = core::mem::align_of::<&str>() as u64;
        cursor = (cursor + align - 1) & !(align - 1);

        let args_slice_ptr = cursor as *mut &str;
        let final_args: &[&str] = unsafe {
            let s = core::slice::from_raw_parts_mut(args_slice_ptr, str_slices.len());
            s.copy_from_slice(&str_slices);
            s
        };

        // Heap mulai setelah args region, dengan gap 4096 bytes
        let heap_start = args_base + map_size as u64 + 4096;
        let heap_size  = ((self.stack_base - heap_start) / 2) as usize;
        unsafe {
            self.allocator.lock().init(heap_start as *mut u8, heap_size);
        }

        set_pid(self.id);

        let entry_vaddr = self.code_base + self.entry_point;
        klog!("CHN: jumping to entry={:#X} stack={:#X} cs={:#X} ss={:#X}",
            entry_vaddr, self.stack_base,
            GDT.1.u_code.0, GDT.1.u_data.0);

        unsafe {
            let (_, flags) = Cr3::read();
            Cr3::write(self.pt_frame, flags);

            jump_to_userspace(
                entry_vaddr,
                self.stack_base,
                GDT.1.u_code.0 as u64,
                GDT.1.u_data.0 as u64,
                final_args.as_ptr() as u64,
                final_args.len() as u64,
            );
        }
    }

    fn load_segment(
        mapper: &mut OffsetPageTable,
        addr:   u64,
        size:   usize,
        data:   &[u8],
    ) -> Result<(), ()> {
        sys::mem::map_page(mapper, addr, size)?;
        unsafe {
            let dst = addr as *mut u8;
            core::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len());
            if size > data.len() {
                core::ptr::write_bytes(dst.add(data.len()), 0, size - data.len());
            }
        }
        Ok(())
    }
}
