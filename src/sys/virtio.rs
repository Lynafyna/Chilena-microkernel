//! VirtIO Block Device Driver untuk Chilena
//!
//! Implementasi VirtIO legacy (pre-1.0) Split Virtqueue untuk block device.
//! Gunakan di QEMU dengan flag:
//!   -drive file=disk.img,if=virtio,format=raw

use crate::sys::pci;
use crate::sys::mem::virt_to_phys;

use core::sync::atomic::{fence, Ordering};
use spin::{Mutex, Once};
use x86_64::instructions::port::Port;
use x86_64::VirtAddr;

// ---------------------------------------------------------------------------
// VirtIO PCI legacy register offsets (dari BAR0 I/O port base)
// ---------------------------------------------------------------------------
const VIRTIO_PCI_HOST_FEATURES:  u16 = 0x00;
const VIRTIO_PCI_GUEST_FEATURES: u16 = 0x04;
const VIRTIO_PCI_QUEUE_PFN:      u16 = 0x08;
const VIRTIO_PCI_QUEUE_SIZE:     u16 = 0x0C;
const VIRTIO_PCI_QUEUE_SEL:      u16 = 0x0E;
const VIRTIO_PCI_QUEUE_NOTIFY:   u16 = 0x10;
const VIRTIO_PCI_STATUS:         u16 = 0x12;

// VirtIO status bits
const VIRTIO_STATUS_RESET:       u8 = 0x00;
const VIRTIO_STATUS_ACKNOWLEDGE: u8 = 0x01;
const VIRTIO_STATUS_DRIVER:      u8 = 0x02;
const VIRTIO_STATUS_DRIVER_OK:   u8 = 0x04;
const VIRTIO_STATUS_FAILED:      u8 = 0x80;

// VirtIO block request types
const VIRTIO_BLK_T_IN:  u32 = 0; // read dari disk
const VIRTIO_BLK_T_OUT: u32 = 1; // tulis ke disk

// Virtqueue descriptor flags
const VIRTQ_DESC_F_NEXT:  u16 = 1;
const VIRTQ_DESC_F_WRITE: u16 = 2; // device boleh tulis ke buffer ini

// Queue size — power of 2
const QUEUE_SIZE: usize = 8;

/// Ukuran satu sektor disk
pub const SECTOR_SIZE: usize = 512;

// ---------------------------------------------------------------------------
// Virtqueue descriptor
// ---------------------------------------------------------------------------
#[repr(C)]
#[derive(Clone, Copy)]
struct VirtqDesc {
    addr:  u64,
    len:   u32,
    flags: u16,
    next:  u16,
}

impl VirtqDesc {
    const fn zero() -> Self {
        Self { addr: 0, len: 0, flags: 0, next: 0 }
    }
}

// ---------------------------------------------------------------------------
// VirtIO block request header
// ---------------------------------------------------------------------------
#[repr(C)]
struct VirtioBlkReq {
    kind:     u32,
    reserved: u32,
    sector:   u64,
}

// ---------------------------------------------------------------------------
// Static memory untuk virtqueue (harus 4096-byte aligned untuk PFN)
// ---------------------------------------------------------------------------

// Kita pack semua virtqueue data dalam satu page-aligned struct
// agar PFN calculation mudah dan benar

#[repr(C, align(4096))]
struct VirtqueueMem {
    // Descriptor table: 16 bytes × QUEUE_SIZE
    desc: [VirtqDesc; QUEUE_SIZE],

    // Available ring: flags(2) + idx(2) + ring(2×N) + used_event(2)
    avail_flags:      u16,
    avail_idx:        u16,
    avail_ring:       [u16; QUEUE_SIZE],
    avail_used_event: u16,

    // Used ring: harus 4096-byte aligned dari awal virtqueue
    // Tambah padding agar used ring mulai di offset 4096
    _pad: [u8; 4096
        - (16 * QUEUE_SIZE)           // desc table
        - (2 + 2 + 2 * QUEUE_SIZE + 2) // avail ring
    ],

    // Used ring
    used_flags:       u16,
    used_idx:         u16,
    used_ring:        [VirtqUsedElem; QUEUE_SIZE],
    used_avail_event: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct VirtqUsedElem {
    id:  u32,
    len: u32,
}

// Static instance — satu virtqueue untuk seluruh driver
static mut VQ_MEM: VirtqueueMem = VirtqueueMem {
    desc:             [VirtqDesc::zero(); QUEUE_SIZE],
    avail_flags:      0,
    avail_idx:        0,
    avail_ring:       [0u16; QUEUE_SIZE],
    avail_used_event: 0,
    _pad:             [0u8; 4096
        - (16 * QUEUE_SIZE)
        - (2 + 2 + 2 * QUEUE_SIZE + 2)
    ],
    used_flags:       0,
    used_idx:         0,
    used_ring:        [VirtqUsedElem { id: 0, len: 0 }; QUEUE_SIZE],
    used_avail_event: 0,
};

// Buffer untuk satu request sekaligus (driver ini single-threaded)
static mut REQ_HDR:    VirtioBlkReq   = VirtioBlkReq { kind: 0, reserved: 0, sector: 0 };
static mut DATA_BUF:   [u8; SECTOR_SIZE] = [0u8; SECTOR_SIZE];
static mut STATUS_BUF: u8             = 0xFF;

// ---------------------------------------------------------------------------
// VirtIO Block device
// ---------------------------------------------------------------------------

struct VirtioBlk {
    io_base:   u16,
    avail_idx: u16, // next slot di available ring
    last_used: u16, // used ring idx yang sudah kita proses
    capacity:  u64, // kapasitas dalam sektor
}

impl VirtioBlk {
    fn read8(&self, reg: u16) -> u8 {
        unsafe { Port::<u8>::new(self.io_base + reg).read() }
    }
    fn read16(&self, reg: u16) -> u16 {
        unsafe { Port::<u16>::new(self.io_base + reg).read() }
    }
    fn read32(&self, reg: u16) -> u32 {
        unsafe { Port::<u32>::new(self.io_base + reg).read() }
    }
    fn write8(&self, reg: u16, v: u8) {
        unsafe { Port::<u8>::new(self.io_base + reg).write(v) }
    }
    fn write16(&self, reg: u16, v: u16) {
        unsafe { Port::<u16>::new(self.io_base + reg).write(v) }
    }
    fn write32(&self, reg: u16, v: u32) {
        unsafe { Port::<u32>::new(self.io_base + reg).write(v) }
    }

    /// Konversi virtual addr ke physical addr
    fn to_phys(vaddr: u64) -> u64 {
        virt_to_phys(VirtAddr::new(vaddr))
            .map(|p| p.as_u64())
            .unwrap_or(vaddr)
    }

    /// Kirim satu block request dan tunggu selesai (polling)
    fn do_request(&mut self, sector: u64, buf: &mut [u8], write: bool) -> Result<(), &'static str> {
        if sector >= self.capacity {
            return Err("virtio: sector out of range");
        }

        unsafe {
            // Setup request header
            REQ_HDR.kind     = if write { VIRTIO_BLK_T_OUT } else { VIRTIO_BLK_T_IN };
            REQ_HDR.reserved = 0;
            REQ_HDR.sector   = sector;
            STATUS_BUF       = 0xFF; // reset status

            if write {
                let n = buf.len().min(SECTOR_SIZE);
                DATA_BUF[..n].copy_from_slice(&buf[..n]);
                if n < SECTOR_SIZE {
                    DATA_BUF[n..].fill(0);
                }
            }

            let slot = (self.avail_idx as usize) % QUEUE_SIZE;

            // Gunakan 3 descriptor chained: header → data → status
            let d0 = (slot * 3) % QUEUE_SIZE;
            let d1 = (slot * 3 + 1) % QUEUE_SIZE;
            let d2 = (slot * 3 + 2) % QUEUE_SIZE;

            VQ_MEM.desc[d0] = VirtqDesc {
                addr:  Self::to_phys(&REQ_HDR as *const _ as u64),
                len:   core::mem::size_of::<VirtioBlkReq>() as u32,
                flags: VIRTQ_DESC_F_NEXT,
                next:  d1 as u16,
            };

            VQ_MEM.desc[d1] = VirtqDesc {
                addr:  Self::to_phys(DATA_BUF.as_ptr() as u64),
                len:   SECTOR_SIZE as u32,
                flags: VIRTQ_DESC_F_NEXT | if write { 0 } else { VIRTQ_DESC_F_WRITE },
                next:  d2 as u16,
            };

            VQ_MEM.desc[d2] = VirtqDesc {
                addr:  Self::to_phys(&STATUS_BUF as *const _ as u64),
                len:   1,
                flags: VIRTQ_DESC_F_WRITE,
                next:  0,
            };

            // Taruh chain head di available ring
            VQ_MEM.avail_ring[slot] = d0 as u16;

            fence(Ordering::SeqCst);

            self.avail_idx = self.avail_idx.wrapping_add(1);
            VQ_MEM.avail_idx = self.avail_idx;

            fence(Ordering::SeqCst);

            // Notify device
            self.write16(VIRTIO_PCI_QUEUE_NOTIFY, 0);

            // Polling tunggu used ring update
            let mut timeout = 2_000_000usize;
            loop {
                fence(Ordering::SeqCst);
                if VQ_MEM.used_idx != self.last_used {
                    self.last_used = self.last_used.wrapping_add(1);
                    break;
                }
                timeout -= 1;
                if timeout == 0 {
                    return Err("virtio: request timeout");
                }
                core::hint::spin_loop();
            }

            if STATUS_BUF != 0 {
                return Err("virtio: request failed (status != 0)");
            }

            if !write {
                let n = buf.len().min(SECTOR_SIZE);
                buf[..n].copy_from_slice(&DATA_BUF[..n]);
            }

            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------

static DEVICE: Once<Mutex<VirtioBlk>> = Once::new();

/// Inisialisasi VirtIO block driver
/// Return true jika berhasil
pub fn init() -> bool {
    // Cari VirtIO block device di PCI bus (coba legacy dulu, lalu modern)
    let dev = pci::find_device(pci::VIRTIO_VENDOR_ID, pci::VIRTIO_BLK_DEVICE_ID_LEGACY)
        .or_else(|| pci::find_device(pci::VIRTIO_VENDOR_ID, pci::VIRTIO_BLK_DEVICE_ID_MODERN));

    let dev = match dev {
        Some(d) => d,
        None => {
            kwarn!("VirtIO: no block device found (add -drive if=virtio to QEMU)");
            return false;
        }
    };

    klog!("VirtIO: block device PCI {:02x}:{:02x}.{}", dev.bus, dev.slot, dev.func);

    // BAR0 bit 0 = 1 → I/O space
    if dev.bar0 & 1 == 0 {
        kerror!("VirtIO: BAR0 is not I/O space, MMIO not supported yet");
        return false;
    }
    let io_base = (dev.bar0 & !0x3) as u16;
    klog!("VirtIO: I/O base = {:#X}", io_base);

    pci::enable_bus_mastering(&dev);

    let mut blk = VirtioBlk { io_base, avail_idx: 0, last_used: 0, capacity: 0 };

    // Sequence init VirtIO legacy (spec v1.0 section 3.1 + legacy compat)
    blk.write8(VIRTIO_PCI_STATUS, VIRTIO_STATUS_RESET);
    for _ in 0..1000 { core::hint::spin_loop(); } // tunggu reset
    blk.write8(VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE);
    blk.write8(VIRTIO_PCI_STATUS, VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER);

    // Nego features — kita tidak minta feature khusus
    let _features = blk.read32(VIRTIO_PCI_HOST_FEATURES);
    blk.write32(VIRTIO_PCI_GUEST_FEATURES, 0);

    // Setup queue 0
    blk.write16(VIRTIO_PCI_QUEUE_SEL, 0);
    let qsize = blk.read16(VIRTIO_PCI_QUEUE_SIZE);
    if qsize == 0 {
        kerror!("VirtIO: queue size = 0, aborting");
        blk.write8(VIRTIO_PCI_STATUS, VIRTIO_STATUS_FAILED);
        return false;
    }
    klog!("VirtIO: queue size = {}", qsize);

    // Beritahu device physical page number dari virtqueue memory
    let vq_phys = VirtioBlk::to_phys(unsafe { &VQ_MEM as *const _ as u64 });
    let pfn = (vq_phys / 4096) as u32;
    blk.write32(VIRTIO_PCI_QUEUE_PFN, pfn);
    klog!("VirtIO: queue PFN = {:#X}", pfn);

    // Driver OK
    blk.write8(
        VIRTIO_PCI_STATUS,
        VIRTIO_STATUS_ACKNOWLEDGE | VIRTIO_STATUS_DRIVER | VIRTIO_STATUS_DRIVER_OK,
    );

    // Baca kapasitas disk dari device config (offset 0x14 dari io_base)
    let cap_lo = unsafe { Port::<u32>::new(io_base + 0x14).read() };
    let cap_hi = unsafe { Port::<u32>::new(io_base + 0x18).read() };
    blk.capacity = ((cap_hi as u64) << 32) | cap_lo as u64;

    klog!("VirtIO: capacity = {} sectors ({} MB)",
        blk.capacity,
        blk.capacity * SECTOR_SIZE as u64 / 1_048_576);

    DEVICE.call_once(|| Mutex::new(blk));
    true
}

/// Apakah VirtIO block device tersedia?
pub fn is_available() -> bool {
    DEVICE.get().is_some()
}

/// Baca satu sektor dari disk ke buf (buf minimal 512 bytes)
pub fn read_sector(sector: u64, buf: &mut [u8]) -> Result<(), &'static str> {
    DEVICE.get()
        .ok_or("virtio: not initialized")?
        .lock()
        .do_request(sector, buf, false)
}

/// Tulis satu sektor dari buf ke disk (buf minimal 512 bytes)
pub fn write_sector(sector: u64, buf: &mut [u8]) -> Result<(), &'static str> {
    DEVICE.get()
        .ok_or("virtio: not initialized")?
        .lock()
        .do_request(sector, buf, true)
}

/// Kapasitas disk dalam sektor
pub fn capacity() -> u64 {
    DEVICE.get().map(|d| d.lock().capacity).unwrap_or(0)
}
