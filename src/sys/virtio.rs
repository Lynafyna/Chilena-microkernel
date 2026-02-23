//! VirtIO Block Device Driver untuk Chilena
//!
//! VirtIO legacy (pre-1.0) Split Virtqueue.
//! FIX: pakai queue size yang device tentukan, bukan hardcode.
//! FIX: DMA buffers dari kernel heap agar virt_to_phys benar.

use crate::sys::pci;
use crate::sys::mem::virt_to_phys;

use core::alloc::Layout;
use core::sync::atomic::{fence, Ordering};
use spin::{Mutex, Once};
use x86_64::instructions::port::Port;
use x86_64::VirtAddr;

// VirtIO PCI legacy register offsets (dari BAR0 I/O port)
const VIRTIO_PCI_HOST_FEATURES:  u16 = 0x00;
const VIRTIO_PCI_GUEST_FEATURES: u16 = 0x04;
const VIRTIO_PCI_QUEUE_PFN:      u16 = 0x08;
const VIRTIO_PCI_QUEUE_SIZE:     u16 = 0x0C;
const VIRTIO_PCI_QUEUE_SEL:      u16 = 0x0E;
const VIRTIO_PCI_QUEUE_NOTIFY:   u16 = 0x10;
const VIRTIO_PCI_STATUS:         u16 = 0x12;

const VIRTIO_STATUS_RESET:       u8 = 0x00;
const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 0x01;
const VIRTIO_STATUS_DRIVER:      u8 = 0x02;
const VIRTIO_STATUS_DRIVER_OK:   u8 = 0x04;
const VIRTIO_STATUS_FAILED:      u8 = 0x80;

const VIRTIO_BLK_T_IN:  u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;

const VIRTQ_DESC_F_NEXT:  u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2;

const VIRTQ_ALIGN: usize = 4096;
pub const SECTOR_SIZE: usize = 512;

// ---------------------------------------------------------------------------
// VirtIO legacy queue layout (semua offset dihitung dari queue_size runtime)
// 
// Offset 0                       : Descriptor Table (16 * queue_size bytes)
// Offset 16*queue_size           : Available Ring (6 + 2*queue_size bytes)
// Offset align(avail_end, 4096)  : Used Ring (6 + 8*queue_size bytes)
// ---------------------------------------------------------------------------

fn desc_table_bytes(qs: usize) -> usize { 16 * qs }
fn avail_ring_offset(qs: usize) -> usize { desc_table_bytes(qs) }
fn avail_ring_bytes(qs: usize)  -> usize { 6 + 2 * qs }
fn used_ring_offset(qs: usize)  -> usize {
    let end = avail_ring_offset(qs) + avail_ring_bytes(qs);
    (end + VIRTQ_ALIGN - 1) & !(VIRTQ_ALIGN - 1)
}
fn used_ring_bytes(qs: usize)   -> usize { 6 + 8 * qs }
fn queue_total_bytes(qs: usize) -> usize {
    used_ring_offset(qs) + used_ring_bytes(qs)
}

// ---------------------------------------------------------------------------
// VirtIO Block request header
// ---------------------------------------------------------------------------
#[repr(C)]
struct VirtioBlkReq { kind: u32, reserved: u32, sector: u64 }

// ---------------------------------------------------------------------------
// VirtIO Block device state
// ---------------------------------------------------------------------------
struct VirtioBlk {
    io_base:    u16,
    queue_size: usize,  // ukuran queue dari device (misal 256)
    avail_idx:  u16,
    last_used:  u16,
    capacity:   u64,

    // DMA buffers (heap-allocated)
    queue_virt:  usize, queue_phys:  u64,  // virtqueue memory
    req_virt:    usize, req_phys:    u64,  // request header
    data_virt:   usize, data_phys:   u64,  // 512-byte data buffer
    status_virt: usize, status_phys: u64,  // 1-byte status
}

impl VirtioBlk {
    fn read8 (&self, r: u16) -> u8  { unsafe { Port::<u8> ::new(self.io_base+r).read() } }
    fn read16(&self, r: u16) -> u16 { unsafe { Port::<u16>::new(self.io_base+r).read() } }
    fn read32(&self, r: u16) -> u32 { unsafe { Port::<u32>::new(self.io_base+r).read() } }
    fn write8 (&self, r: u16, v: u8)  { unsafe { Port::<u8> ::new(self.io_base+r).write(v) } }
    fn write16(&self, r: u16, v: u16) { unsafe { Port::<u16>::new(self.io_base+r).write(v) } }
    fn write32(&self, r: u16, v: u32) { unsafe { Port::<u32>::new(self.io_base+r).write(v) } }

    // Tulis descriptor ke descriptor table
    unsafe fn write_desc(&self, idx: usize, addr: u64, len: u32, flags: u16, next: u16) {
        let base = self.queue_virt + idx * 16; // desc table di offset 0
        (base           as *mut u64).write_volatile(addr);
        ((base + 8)     as *mut u32).write_volatile(len);
        ((base + 12)    as *mut u16).write_volatile(flags);
        ((base + 14)    as *mut u16).write_volatile(next);
    }

    // Baca used ring idx
    unsafe fn used_idx(&self) -> u16 {
        // Used ring: flags(2) + idx(2) + ring[...]
        let ptr = (self.queue_virt + used_ring_offset(self.queue_size) + 2) as *const u16;
        ptr.read_volatile()
    }

    fn do_request(&mut self, sector: u64, buf: &mut [u8], write: bool) -> Result<(), &'static str> {
        if sector >= self.capacity {
            return Err("virtio: sector out of range");
        }

        unsafe {
            // Setup request header
            let req = self.req_virt as *mut VirtioBlkReq;
            (*req).kind     = if write { VIRTIO_BLK_T_OUT } else { VIRTIO_BLK_T_IN };
            (*req).reserved = 0;
            (*req).sector   = sector;

            // Setup data buffer
            if write {
                let dst = self.data_virt as *mut u8;
                let n = buf.len().min(SECTOR_SIZE);
                core::ptr::copy_nonoverlapping(buf.as_ptr(), dst, n);
                if n < SECTOR_SIZE {
                    core::ptr::write_bytes(dst.add(n), 0, SECTOR_SIZE - n);
                }
            }

            // Reset status
            (self.status_virt as *mut u8).write_volatile(0xFF);

            // Setup 3 descriptors: hdr(0) → data(1) → status(2)
            self.write_desc(0, self.req_phys,
                core::mem::size_of::<VirtioBlkReq>() as u32,
                VIRTQ_DESC_F_NEXT, 1);
            self.write_desc(1, self.data_phys,
                SECTOR_SIZE as u32,
                VIRTQ_DESC_F_NEXT | if write { 0 } else { VIRTQ_DESC_F_WRITE },
                2);
            self.write_desc(2, self.status_phys, 1,
                VIRTQ_DESC_F_WRITE, 0);

            // Tulis ke available ring
            // Avail ring layout: flags(2) + idx(2) + ring[qs*2] + used_event(2)
            let qs = self.queue_size;
            let avail_base = self.queue_virt + avail_ring_offset(qs);
            let slot = (self.avail_idx as usize) % qs;
            // ring[slot] = 0 (chain head di descriptor 0)
            let ring_ptr = (avail_base + 4 + slot * 2) as *mut u16;
            ring_ptr.write_volatile(0);

            fence(Ordering::SeqCst);

            // Update avail idx
            self.avail_idx = self.avail_idx.wrapping_add(1);
            ((avail_base + 2) as *mut u16).write_volatile(self.avail_idx);

            fence(Ordering::SeqCst);

            // Notify device (queue 0)
            self.write16(VIRTIO_PCI_QUEUE_NOTIFY, 0);

            // Poll used ring
            let mut timeout = 10_000_000usize;
            loop {
                fence(Ordering::SeqCst);
                if self.used_idx() != self.last_used {
                    self.last_used = self.last_used.wrapping_add(1);
                    break;
                }
                timeout -= 1;
                if timeout == 0 {
                    return Err("virtio: request timeout");
                }
                core::hint::spin_loop();
            }

            // Cek status
            let status = (self.status_virt as *const u8).read_volatile();
            if status != 0 {
                return Err("virtio: device error");
            }

            // Copy data ke caller jika read
            if !write {
                let src = self.data_virt as *const u8;
                let n = buf.len().min(SECTOR_SIZE);
                core::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), n);
            }

            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// DMA allocation helper
// ---------------------------------------------------------------------------
fn alloc_dma(size: usize, align: usize) -> Option<(usize, u64)> {
    let layout = Layout::from_size_align(size, align).ok()?;
    let virt = unsafe { alloc::alloc::alloc_zeroed(layout) } as usize;
    if virt == 0 { return None; }
    let phys = virt_to_phys(VirtAddr::new(virt as u64))?.as_u64();
    klog!("VirtIO: alloc virt={:#X} phys={:#X} size={}", virt, phys, size);
    Some((virt, phys))
}

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------
static DEVICE: Once<Mutex<VirtioBlk>> = Once::new();

pub fn init() -> bool {
    let dev = pci::find_device(pci::VIRTIO_VENDOR_ID, pci::VIRTIO_BLK_DEVICE_ID_LEGACY)
        .or_else(|| pci::find_device(pci::VIRTIO_VENDOR_ID, pci::VIRTIO_BLK_DEVICE_ID_MODERN));

    let dev = match dev {
        Some(d) => d,
        None => { kwarn!("VirtIO: no block device found"); return false; }
    };

    klog!("VirtIO: PCI {:02x}:{:02x}.{}", dev.bus, dev.slot, dev.func);

    if dev.bar0 & 1 == 0 {
        kerror!("VirtIO: BAR0 not I/O space"); return false;
    }
    let io_base = (dev.bar0 & !0x3) as u16;
    klog!("VirtIO: I/O base = {:#X}", io_base);

    pci::enable_bus_mastering(&dev);

    // Baca queue size dari device DULU sebelum alokasi
    let tmp = VirtioBlk {
        io_base, queue_size: 0, avail_idx: 0, last_used: 0, capacity: 0,
        queue_virt: 0, queue_phys: 0, req_virt: 0, req_phys: 0,
        data_virt: 0, data_phys: 0, status_virt: 0, status_phys: 0,
    };

    // Init sequence sebagian untuk baca queue size
    tmp.write8(VIRTIO_PCI_STATUS, VIRTIO_STATUS_RESET);
    for _ in 0..10000 { core::hint::spin_loop(); }
    tmp.write8(VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE);
    tmp.write8(VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER);
    let _feats = tmp.read32(VIRTIO_PCI_HOST_FEATURES);
    tmp.write32(VIRTIO_PCI_GUEST_FEATURES, 0);
    tmp.write16(VIRTIO_PCI_QUEUE_SEL, 0);

    let qs = tmp.read16(VIRTIO_PCI_QUEUE_SIZE) as usize;
    if qs == 0 { kerror!("VirtIO: queue size = 0"); return false; }
    klog!("VirtIO: queue size = {}", qs);

    // Hitung ukuran queue yang benar berdasarkan qs dari device
    let qtotal = queue_total_bytes(qs);
    klog!("VirtIO: queue memory needed = {} bytes (used_ring at offset {})",
        qtotal, used_ring_offset(qs));

    // Alokasi DMA buffers
    let (qv, qp) = match alloc_dma(qtotal, VIRTQ_ALIGN) {
        Some(x) => x, None => { kerror!("VirtIO: alloc queue failed"); return false; }
    };
    let (rv, rp) = match alloc_dma(core::mem::size_of::<VirtioBlkReq>(), 16) {
        Some(x) => x, None => { kerror!("VirtIO: alloc req failed"); return false; }
    };
    let (dv, dp) = match alloc_dma(SECTOR_SIZE, SECTOR_SIZE) {
        Some(x) => x, None => { kerror!("VirtIO: alloc data failed"); return false; }
    };
    let (sv, sp) = match alloc_dma(1, 1) {
        Some(x) => x, None => { kerror!("VirtIO: alloc status failed"); return false; }
    };

    // Beritahu device PFN (queue_phys / 4096)
    let pfn = (qp / 4096) as u32;
    tmp.write32(VIRTIO_PCI_QUEUE_PFN, pfn);
    klog!("VirtIO: PFN = {:#X} (phys = {:#X})", pfn, qp);

    // Driver OK
    tmp.write8(VIRTIO_PCI_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK);

    // Baca kapasitas
    let cap_lo = unsafe { Port::<u32>::new(io_base + 0x14).read() };
    let cap_hi = unsafe { Port::<u32>::new(io_base + 0x18).read() };
    let capacity = ((cap_hi as u64) << 32) | cap_lo as u64;
    klog!("VirtIO: capacity = {} sectors ({} MB)",
        capacity, capacity * SECTOR_SIZE as u64 / 1_048_576);

    DEVICE.call_once(|| Mutex::new(VirtioBlk {
        io_base, queue_size: qs,
        avail_idx: 0, last_used: 0, capacity,
        queue_virt: qv, queue_phys: qp,
        req_virt: rv,   req_phys: rp,
        data_virt: dv,  data_phys: dp,
        status_virt: sv, status_phys: sp,
    }));
    true
}

pub fn is_available() -> bool { DEVICE.get().is_some() }

pub fn read_sector(sector: u64, buf: &mut [u8]) -> Result<(), &'static str> {
    DEVICE.get().ok_or("virtio: not init")?.lock().do_request(sector, buf, false)
}

pub fn write_sector(sector: u64, buf: &mut [u8]) -> Result<(), &'static str> {
    DEVICE.get().ok_or("virtio: not init")?.lock().do_request(sector, buf, true)
}

pub fn capacity() -> u64 {
    DEVICE.get().map(|d| d.lock().capacity).unwrap_or(0)
}
