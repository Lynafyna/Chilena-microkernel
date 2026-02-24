//! ChilenaFS — Custom filesystem untuk Chilena
//!
//! Format on-disk yang sepenuhnya custom, tidak bergantung ke Linux/Windows.
//!
//! LAYOUT DISK:
//!   Sektor 0      : Superblock
//!   Sektor 1-8    : Inode Table (64 inode × 64 bytes = 8 sektor)
//!   Sektor 9+     : Data blocks (isi file)
//!
//! SUPERBLOCK (512 bytes):
//!   [0..4]   magic       = 0x43484653 ('C','H','F','S')
//!   [4..8]   version     = 1
//!   [8..12]  inode_count = jumlah inode terpakai
//!   [12..16] data_start  = 9 (sektor pertama data)
//!   [16..512] reserved
//!
//! INODE (64 bytes):
//!   [0]      flags       = 0 (free) / 1 (file) / 2 (dir)
//!   [1..49]  name        = nama file/dir (48 bytes, null-terminated)
//!   [49..53] size        = ukuran dalam bytes (u32)
//!   [53..57] start_sector= sektor pertama data (u32)
//!   [57..59] block_count = jumlah sektor yang dipakai (u16)
//!   [59..64] reserved

use crate::sys::virtio;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use spin::Mutex;

// ---------------------------------------------------------------------------
// Konstanta layout
// ---------------------------------------------------------------------------

pub const MAGIC: u32          = 0x43484653; // 'CHFS'
pub const VERSION: u32        = 1;
pub const SECTOR_SIZE: usize  = 512;
pub const INODE_SIZE: usize   = 64;
pub const INODES_PER_SECTOR: usize = SECTOR_SIZE / INODE_SIZE; // 8
pub const INODE_SECTORS: usize     = 8;
pub const MAX_INODES: usize        = INODES_PER_SECTOR * INODE_SECTORS; // 64

pub const SUPERBLOCK_SECTOR: u64 = 0;
pub const INODE_TABLE_START:  u64 = 1;
pub const DATA_START:         u64 = 9; // sektor pertama data

// Inode flags
pub const INODE_FREE: u8 = 0;
pub const INODE_FILE: u8 = 1;
pub const INODE_DIR:  u8 = 2;

// ---------------------------------------------------------------------------
// On-disk structs — repr(C) agar layout deterministik
// ---------------------------------------------------------------------------

/// Superblock — sektor 0
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Superblock {
    pub magic:       u32,
    pub version:     u32,
    pub inode_count: u32,  // jumlah inode yang terpakai
    pub data_start:  u32,  // selalu 9
    _reserved:       [u8; SECTOR_SIZE - 16],
}

impl Superblock {
    pub fn new() -> Self {
        Self {
            magic:       MAGIC,
            version:     VERSION,
            inode_count: 0,
            data_start:  DATA_START as u32,
            _reserved:   [0u8; SECTOR_SIZE - 16],
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC && self.version == VERSION
    }

    pub fn to_bytes(&self) -> [u8; SECTOR_SIZE] {
        let mut buf = [0u8; SECTOR_SIZE];
        buf[0..4].copy_from_slice(&self.magic.to_le_bytes());
        buf[4..8].copy_from_slice(&self.version.to_le_bytes());
        buf[8..12].copy_from_slice(&self.inode_count.to_le_bytes());
        buf[12..16].copy_from_slice(&self.data_start.to_le_bytes());
        buf
    }

    pub fn from_bytes(buf: &[u8; SECTOR_SIZE]) -> Self {
        let magic       = u32::from_le_bytes(buf[0..4].try_into().unwrap_or([0;4]));
        let version     = u32::from_le_bytes(buf[4..8].try_into().unwrap_or([0;4]));
        let inode_count = u32::from_le_bytes(buf[8..12].try_into().unwrap_or([0;4]));
        let data_start  = u32::from_le_bytes(buf[12..16].try_into().unwrap_or([0;4]));
        Self { magic, version, inode_count, data_start, _reserved: [0u8; SECTOR_SIZE-16] }
    }
}

/// Inode — 64 bytes per entry
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Inode {
    pub flags:        u8,
    pub name:         [u8; 48],  // null-terminated filename
    pub size:         u32,       // bytes
    pub start_sector: u32,       // sektor pertama data
    pub block_count:  u16,       // jumlah sektor
    _reserved:        [u8; 5],
}

impl Inode {
    pub const fn empty() -> Self {
        Self {
            flags: INODE_FREE,
            name: [0u8; 48],
            size: 0,
            start_sector: 0,
            block_count: 0,
            _reserved: [0u8; 5],
        }
    }

    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(48);
        core::str::from_utf8(&self.name[..end]).unwrap_or("")
    }

    pub fn set_name(&mut self, s: &str) {
        self.name = [0u8; 48];
        let n = s.len().min(47);
        self.name[..n].copy_from_slice(&s.as_bytes()[..n]);
    }

    pub fn to_bytes(&self) -> [u8; INODE_SIZE] {
        let mut buf = [0u8; INODE_SIZE];
        buf[0]      = self.flags;
        buf[1..49].copy_from_slice(&self.name);
        buf[49..53].copy_from_slice(&self.size.to_le_bytes());
        buf[53..57].copy_from_slice(&self.start_sector.to_le_bytes());
        buf[57..59].copy_from_slice(&self.block_count.to_le_bytes());
        buf
    }

    pub fn from_bytes(buf: &[u8; INODE_SIZE]) -> Self {
        let mut name = [0u8; 48];
        name.copy_from_slice(&buf[1..49]);
        let size         = u32::from_le_bytes(buf[49..53].try_into().unwrap_or([0;4]));
        let start_sector = u32::from_le_bytes(buf[53..57].try_into().unwrap_or([0;4]));
        let block_count  = u16::from_le_bytes(buf[57..59].try_into().unwrap_or([0;2]));
        Self { flags: buf[0], name, size, start_sector, block_count, _reserved: [0u8; 5] }
    }
}

// ---------------------------------------------------------------------------
// Info untuk userspace (tidak ada pointer ke disk)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct FileInfo {
    pub name:     String,
    pub size:     usize,
    pub is_dir:   bool,
    pub inode_id: usize,
}

// ---------------------------------------------------------------------------
// ChilenaFS state
// ---------------------------------------------------------------------------

struct ChfsState {
    mounted:       bool,
    superblock:    Superblock,
    next_sector:   u64,  // sektor data berikutnya yang bebas
}

impl ChfsState {
    const fn new() -> Self {
        Self {
            mounted:     false,
            superblock:  Superblock {
                magic: 0, version: 0, inode_count: 0,
                data_start: 0, _reserved: [0u8; SECTOR_SIZE - 16],
            },
            next_sector: DATA_START,
        }
    }
}

static CHFS: Mutex<ChfsState> = Mutex::new(ChfsState::new());

// ---------------------------------------------------------------------------
// Low-level disk I/O helpers
// ---------------------------------------------------------------------------

fn read_sector(sector: u64) -> Result<[u8; SECTOR_SIZE], &'static str> {
    let mut buf = [0u8; SECTOR_SIZE];
    virtio::read_sector(sector, &mut buf)?;
    Ok(buf)
}

fn write_sector(sector: u64, buf: &[u8; SECTOR_SIZE]) -> Result<(), &'static str> {
    let mut b = *buf;
    virtio::write_sector(sector, &mut b)
}

/// Baca satu inode dari inode table
fn read_inode(id: usize) -> Result<Inode, &'static str> {
    if id >= MAX_INODES { return Err("chfs: inode id out of range"); }
    let sector = INODE_TABLE_START + (id / INODES_PER_SECTOR) as u64;
    let offset = (id % INODES_PER_SECTOR) * INODE_SIZE;
    let buf = read_sector(sector)?;
    let mut ibuf = [0u8; INODE_SIZE];
    ibuf.copy_from_slice(&buf[offset..offset + INODE_SIZE]);
    Ok(Inode::from_bytes(&ibuf))
}

/// Tulis satu inode ke inode table
fn write_inode(id: usize, inode: &Inode) -> Result<(), &'static str> {
    if id >= MAX_INODES { return Err("chfs: inode id out of range"); }
    let sector = INODE_TABLE_START + (id / INODES_PER_SECTOR) as u64;
    let offset = (id % INODES_PER_SECTOR) * INODE_SIZE;
    let mut buf = read_sector(sector)?;
    buf[offset..offset + INODE_SIZE].copy_from_slice(&inode.to_bytes());
    write_sector(sector, &buf)
}

/// Cari inode bebas, return index-nya
fn find_free_inode() -> Option<usize> {
    for i in 0..MAX_INODES {
        if let Ok(inode) = read_inode(i) {
            if inode.flags == INODE_FREE { return Some(i); }
        }
    }
    None
}

/// Cari inode berdasarkan path (nama file/dir)
fn find_inode_by_path(path: &str) -> Option<(usize, Inode)> {
    let name = basename(path);
    for i in 0..MAX_INODES {
        if let Ok(inode) = read_inode(i) {
            if inode.flags != INODE_FREE && inode.name_str() == name {
                return Some((i, inode));
            }
        }
    }
    None
}

/// Ambil nama file dari path (bagian setelah '/' terakhir)
fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Format disk dengan ChilenaFS baru (hapus semua data!)
pub fn format() -> Result<(), &'static str> {
    if !virtio::is_available() {
        return Err("chfs: VirtIO not available");
    }

    klog!("ChilenaFS: formatting disk...");

    // Tulis superblock
    let sb = Superblock::new();
    let sb_bytes = sb.to_bytes();
    write_sector(SUPERBLOCK_SECTOR, &sb_bytes)?;

    // Kosongkan semua sektor inode table
    let empty = [0u8; SECTOR_SIZE];
    for s in INODE_TABLE_START..DATA_START {
        write_sector(s, &empty)?;
    }

    // Update state
    let mut state = CHFS.lock();
    state.mounted     = true;
    state.superblock  = sb;
    state.next_sector = DATA_START;

    klog!("ChilenaFS: format complete (max {} files)", MAX_INODES);
    Ok(())
}

/// Mount ChilenaFS dari disk yang sudah diformat
/// Return: true jika berhasil, false jika disk belum diformat
pub fn mount() -> bool {
    if !virtio::is_available() {
        return false;
    }

    let buf = match read_sector(SUPERBLOCK_SECTOR) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let sb = Superblock::from_bytes(&buf);
    if !sb.is_valid() {
        kwarn!("ChilenaFS: disk not formatted (magic={:#X})", sb.magic);
        return false;
    }

    // Hitung next_sector dari inode table
    let mut next_sector = DATA_START;
    for i in 0..MAX_INODES {
        if let Ok(inode) = read_inode(i) {
            if inode.flags != INODE_FREE {
                let end = inode.start_sector as u64 + inode.block_count as u64;
                if end > next_sector { next_sector = end; }
            }
        }
    }

    let mut state = CHFS.lock();
    state.mounted     = true;
    state.superblock  = sb;
    state.next_sector = next_sector;

    klog!("ChilenaFS: mounted OK ({} files, next_sector={})",
        sb.inode_count, next_sector);
    true
}

pub fn is_mounted() -> bool {
    CHFS.lock().mounted
}

/// Buat file baru atau timpa file yang sudah ada
pub fn write_file(path: &str, data: &[u8]) -> Result<(), &'static str> {
    if !is_mounted() { return Err("chfs: not mounted"); }

    let name = basename(path);
    if name.len() > 47 { return Err("chfs: filename too long (max 47)"); }

    // Hitung berapa sektor yang dibutuhkan
    let block_count = ((data.len() + SECTOR_SIZE - 1) / SECTOR_SIZE).max(1) as u16;

    // Cek apakah file sudah ada → hapus dulu (overwrite)
    if let Some((id, _)) = find_inode_by_path(path) {
        // Hapus inode lama (data lama di disk dibiarkan, sektor baru dialokasi)
        let mut inode = Inode::empty();
        write_inode(id, &inode)?;
        // Update superblock count
        let mut state = CHFS.lock();
        if state.superblock.inode_count > 0 {
            state.superblock.inode_count -= 1;
        }
    }

    // Alokasi inode baru
    let inode_id = find_free_inode().ok_or("chfs: no free inodes")?;

    let start_sector = {
        let state = CHFS.lock();
        state.next_sector
    };

    // Tulis data ke disk sector by sector
    for i in 0..block_count as usize {
        let mut sector_buf = [0u8; SECTOR_SIZE];
        let src_start = i * SECTOR_SIZE;
        let src_end   = (src_start + SECTOR_SIZE).min(data.len());
        if src_start < data.len() {
            sector_buf[..src_end - src_start].copy_from_slice(&data[src_start..src_end]);
        }
        write_sector(start_sector + i as u64, &sector_buf)?;
    }

    // Tulis inode
    let mut inode = Inode::empty();
    inode.flags = INODE_FILE;
    inode.set_name(name);
    inode.size         = data.len() as u32;
    inode.start_sector = start_sector as u32;
    inode.block_count  = block_count;
    write_inode(inode_id, &inode)?;

    // Update superblock
    {
        let mut state = CHFS.lock();
        state.superblock.inode_count += 1;
        state.next_sector = start_sector + block_count as u64;
        let sb_bytes = state.superblock.to_bytes();
        drop(state); // release lock sebelum disk write
        write_sector(SUPERBLOCK_SECTOR, &sb_bytes)?;
    }

    klog!("ChilenaFS: write '{}' {} bytes @ sector {}",
        name, data.len(), start_sector);
    Ok(())
}

/// Buat direktori
pub fn mkdir(path: &str) -> Result<(), &'static str> {
    if !is_mounted() { return Err("chfs: not mounted"); }

    let name = basename(path);
    if name.len() > 47 { return Err("chfs: dirname too long"); }

    // Kalau sudah ada, skip
    if find_inode_by_path(path).is_some() { return Ok(()); }

    let inode_id = find_free_inode().ok_or("chfs: no free inodes")?;

    let mut inode = Inode::empty();
    inode.flags = INODE_DIR;
    inode.set_name(name);
    inode.size         = 0;
    inode.start_sector = 0;
    inode.block_count  = 0;
    write_inode(inode_id, &inode)?;

    let mut state = CHFS.lock();
    state.superblock.inode_count += 1;
    let sb_bytes = state.superblock.to_bytes();
    drop(state);
    write_sector(SUPERBLOCK_SECTOR, &sb_bytes)?;

    klog!("ChilenaFS: mkdir '{}'", name);
    Ok(())
}

/// Baca isi file, return Vec<u8>
pub fn read_file(path: &str) -> Result<Vec<u8>, &'static str> {
    if !is_mounted() { return Err("chfs: not mounted"); }

    let (_, inode) = find_inode_by_path(path).ok_or("chfs: file not found")?;

    if inode.flags != INODE_FILE {
        return Err("chfs: not a file");
    }

    let mut data = Vec::new();
    let total_bytes = inode.size as usize;

    for i in 0..inode.block_count as usize {
        let sector = inode.start_sector as u64 + i as u64;
        let buf = read_sector(sector)?;
        let remaining = total_bytes - data.len();
        let take = remaining.min(SECTOR_SIZE);
        data.extend_from_slice(&buf[..take]);
    }

    Ok(data)
}

/// Cek apakah file/dir ada
pub fn exists(path: &str) -> bool {
    if !is_mounted() { return false; }
    find_inode_by_path(path).is_some()
}

/// Hapus file
pub fn remove(path: &str) -> Result<(), &'static str> {
    if !is_mounted() { return Err("chfs: not mounted"); }

    let (id, _) = find_inode_by_path(path).ok_or("chfs: file not found")?;
    let empty = Inode::empty();
    write_inode(id, &empty)?;

    let mut state = CHFS.lock();
    if state.superblock.inode_count > 0 {
        state.superblock.inode_count -= 1;
    }
    let sb_bytes = state.superblock.to_bytes();
    drop(state);
    write_sector(SUPERBLOCK_SECTOR, &sb_bytes)?;

    Ok(())
}

/// List semua file/dir
pub fn list_all() -> Vec<FileInfo> {
    let mut result = Vec::new();
    if !is_mounted() { return result; }

    for i in 0..MAX_INODES {
        if let Ok(inode) = read_inode(i) {
            if inode.flags != INODE_FREE {
                result.push(FileInfo {
                    name:     inode.name_str().to_string(),
                    size:     inode.size as usize,
                    is_dir:   inode.flags == INODE_DIR,
                    inode_id: i,
                });
            }
        }
    }
    result
}

/// Info superblock untuk debug
pub fn info() -> (u32, u32, u64) {
    let state = CHFS.lock();
    (state.superblock.inode_count,
     state.superblock.data_start,
     state.next_sector)
}
