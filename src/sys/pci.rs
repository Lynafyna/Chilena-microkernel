//! PCI Bus Scanner untuk Chilena
//!
//! Scan PCI bus untuk menemukan device yang terdaftar.
//! Digunakan oleh VirtIO driver untuk menemukan block device.

use x86_64::instructions::port::Port;

// PCI config space ports
const PCI_CONFIG_ADDRESS: u16 = 0xCF8;
const PCI_CONFIG_DATA:    u16 = 0xCFC;

// VirtIO vendor ID
pub const VIRTIO_VENDOR_ID: u16 = 0x1AF4;

// VirtIO device IDs
pub const VIRTIO_BLK_DEVICE_ID_LEGACY: u16 = 0x1001; // legacy
pub const VIRTIO_BLK_DEVICE_ID_MODERN: u16 = 0x1042; // modern (1.0+)

/// Info PCI device yang ditemukan
#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus:       u8,
    pub slot:      u8,
    pub func:      u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class:     u8,
    pub subclass:  u8,
    pub bar0:      u32, // Base Address Register 0
    pub irq_line:  u8,
}

/// Baca 32-bit dari PCI config space
pub fn config_read32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let addr: u32 = (1 << 31)
        | ((bus  as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset & 0xFC) as u32);

    unsafe {
        let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
        let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);
        addr_port.write(addr);
        data_port.read()
    }
}

/// Tulis 32-bit ke PCI config space
pub fn config_write32(bus: u8, slot: u8, func: u8, offset: u8, value: u32) {
    let addr: u32 = (1 << 31)
        | ((bus  as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) << 8)
        | ((offset & 0xFC) as u32);

    unsafe {
        let mut addr_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
        let mut data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);
        addr_port.write(addr);
        data_port.write(value);
    }
}

/// Baca 16-bit dari PCI config space
pub fn config_read16(bus: u8, slot: u8, func: u8, offset: u8) -> u16 {
    let val = config_read32(bus, slot, func, offset & !2);
    if offset & 2 == 0 {
        (val & 0xFFFF) as u16
    } else {
        (val >> 16) as u16
    }
}

/// Baca 8-bit dari PCI config space
pub fn config_read8(bus: u8, slot: u8, func: u8, offset: u8) -> u8 {
    let val = config_read32(bus, slot, func, offset & !3);
    ((val >> ((offset & 3) * 8)) & 0xFF) as u8
}

/// Scan semua PCI bus dan temukan device dengan vendor_id dan device_id tertentu
pub fn find_device(vendor_id: u16, device_id: u16) -> Option<PciDevice> {
    for bus in 0u8..=255 {
        for slot in 0u8..32 {
            for func in 0u8..8 {
                let vid = config_read16(bus, slot, func, 0x00);
                if vid == 0xFFFF {
                    // Tidak ada device
                    if func == 0 { break; }
                    continue;
                }

                let did = config_read16(bus, slot, func, 0x02);

                if vid == vendor_id && did == device_id {
                    let class_info = config_read32(bus, slot, func, 0x08);
                    let bar0       = config_read32(bus, slot, func, 0x10);
                    let irq_line   = config_read8(bus, slot, func, 0x3C);

                    return Some(PciDevice {
                        bus, slot, func,
                        vendor_id: vid,
                        device_id: did,
                        class:    ((class_info >> 24) & 0xFF) as u8,
                        subclass: ((class_info >> 16) & 0xFF) as u8,
                        bar0,
                        irq_line,
                    });
                }

                // Kalau bukan multi-function, skip func 1-7
                if func == 0 {
                    let header = config_read8(bus, slot, 0, 0x0E);
                    if header & 0x80 == 0 { break; }
                }
            }
        }
    }
    None
}

/// Enable Bus Mastering di PCI device (dibutuhkan untuk DMA)
pub fn enable_bus_mastering(dev: &PciDevice) {
    let cmd = config_read16(dev.bus, dev.slot, dev.func, 0x04);
    config_write32(
        dev.bus, dev.slot, dev.func, 0x04,
        (cmd | 0x0004) as u32, // set Bus Master bit
    );
}
