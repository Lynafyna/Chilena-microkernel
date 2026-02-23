//! Protokol IPC DiskServer untuk Chilena
//!
//! Digunakan oleh:
//!   - usr/disk/server.rs  (DiskServer — menerima request)
//!   - usr/disk/client.rs  (DiskClient — mengirim request dari shell)
//!
//! Karena IPC Message payload = 64 bytes, sektor 512 bytes dikirim
//! dalam 9 chunk (8×60 bytes data + 1 chunk header/status).

// ---------------------------------------------------------------------------
// Konstanta protokol
// ---------------------------------------------------------------------------

/// PID yang selalu digunakan DiskServer (spawn pertama saat boot)
pub const DISK_SERVER_PID: usize = 1;

/// Ukuran chunk data per message
pub const CHUNK_DATA_SIZE: usize = 56; // 64 - 8 bytes header

/// Jumlah chunk untuk 1 sektor (512 bytes)
pub const CHUNKS_PER_SECTOR: usize = (512 + CHUNK_DATA_SIZE - 1) / CHUNK_DATA_SIZE; // = 10

// ---------------------------------------------------------------------------
// Message kinds (kind field di IPC Message)
// ---------------------------------------------------------------------------

/// Client → Server: Ping untuk cek apakah server hidup
pub const MSG_PING: u32 = 0x01;

/// Client → Server: Baca satu sektor
/// data[0..8] = sector number (u64 little-endian)
pub const MSG_READ: u32 = 0x02;

/// Client → Server: Tulis satu sektor
/// data[0..8] = sector number (u64 little-endian)
/// Diikuti CHUNKS_PER_SECTOR message MSG_WRITE_CHUNK
pub const MSG_WRITE: u32 = 0x03;

/// Client → Server: Chunk data untuk write
/// data[0..2] = chunk index (u16)
/// data[2..2+CHUNK_DATA_SIZE] = chunk data
pub const MSG_WRITE_CHUNK: u32 = 0x04;

/// Client → Server: Tanya kapasitas disk
pub const MSG_CAPACITY: u32 = 0x05;

/// Server → Client: Pong (response untuk PING)
pub const MSG_PONG: u32 = 0x81;

/// Server → Client: Satu chunk data hasil read
/// data[0..2] = chunk index (u16)
/// data[2..2+CHUNK_DATA_SIZE] = chunk data
pub const MSG_READ_CHUNK: u32 = 0x82;

/// Server → Client: Selesai (semua chunk sudah dikirim)
/// data[0] = 0 (ok) atau 1 (error)
pub const MSG_DONE: u32 = 0x83;

/// Server → Client: Kapasitas disk
/// data[0..8] = sectors (u64)
pub const MSG_CAPACITY_REPLY: u32 = 0x84;

/// Server → Client: Error
/// data[0..N] = error message string
pub const MSG_ERROR: u32 = 0xFF;

// ---------------------------------------------------------------------------
// Helper: encode/decode sector number dari payload bytes
// ---------------------------------------------------------------------------

pub fn encode_u64(val: u64) -> [u8; 8] {
    val.to_le_bytes()
}

pub fn decode_u64(bytes: &[u8]) -> u64 {
    let mut arr = [0u8; 8];
    let n = bytes.len().min(8);
    arr[..n].copy_from_slice(&bytes[..n]);
    u64::from_le_bytes(arr)
}

pub fn encode_u16(val: u16) -> [u8; 2] {
    val.to_le_bytes()
}

pub fn decode_u16(bytes: &[u8]) -> u16 {
    let mut arr = [0u8; 2];
    let n = bytes.len().min(2);
    arr[..n].copy_from_slice(&bytes[..n]);
    u16::from_le_bytes(arr)
}
