//! DiskClient — API untuk berkomunikasi dengan DiskServer via IPC
//!
//! Digunakan oleh shell commands dan userspace programs.
//! Semua komunikasi lewat IPC send/recv — tidak ada syscall disk langsung.

use crate::api::syscall;
use crate::sys::ipc::Message;
use super::proto::*;

/// Cek apakah DiskServer hidup dan VirtIO tersedia
/// Return: kapasitas disk dalam sektor, atau None jika gagal
pub fn ping() -> Option<u64> {
    let mut msg = Message::empty();
    let r = syscall::send(DISK_SERVER_PID, MSG_PING, &[]);
    if r == usize::MAX { return None; }

    syscall::recv(&mut msg);
    if msg.kind == MSG_PONG {
        Some(decode_u64(&msg.data[..8]))
    } else {
        None
    }
}

/// Tanya kapasitas disk dalam sektor
pub fn capacity() -> Option<u64> {
    let mut msg = Message::empty();
    let r = syscall::send(DISK_SERVER_PID, MSG_CAPACITY, &[]);
    if r == usize::MAX { return None; }

    syscall::recv(&mut msg);
    if msg.kind == MSG_CAPACITY_REPLY {
        Some(decode_u64(&msg.data[..8]))
    } else {
        None
    }
}

/// Baca satu sektor (512 bytes) dari disk
/// Return: Ok(data 512 bytes) atau Err(pesan error)
pub fn read_sector(sector: u64) -> Result<[u8; 512], &'static str> {
    // Kirim request
    let req = encode_u64(sector);
    let r = syscall::send(DISK_SERVER_PID, MSG_READ, &req);
    if r == usize::MAX {
        return Err("disk: failed to send request to DiskServer");
    }

    // Terima chunks
    let mut buf = [0u8; 512];
    let mut msg = Message::empty();

    loop {
        syscall::recv(&mut msg);

        match msg.kind {
            MSG_READ_CHUNK => {
                let idx   = decode_u16(&msg.data[..2]) as usize;
                let start = idx * CHUNK_DATA_SIZE;
                let end   = (start + CHUNK_DATA_SIZE).min(512);
                if start < 512 {
                    let len = end - start;
                    buf[start..end].copy_from_slice(&msg.data[2..2 + len]);
                }
            }
            MSG_DONE => {
                if msg.data[0] == 0 {
                    return Ok(buf);
                } else {
                    return Err("disk: server returned error");
                }
            }
            MSG_ERROR => {
                return Err("disk: server error");
            }
            _ => {
                return Err("disk: unexpected message from server");
            }
        }
    }
}

/// Tulis satu sektor (512 bytes) ke disk
pub fn write_sector(sector: u64, data: &[u8; 512]) -> Result<(), &'static str> {
    // Kirim request header
    let req = encode_u64(sector);
    let r = syscall::send(DISK_SERVER_PID, MSG_WRITE, &req);
    if r == usize::MAX {
        return Err("disk: failed to send write request");
    }

    // Kirim data dalam chunks
    for i in 0..CHUNKS_PER_SECTOR {
        let start = i * CHUNK_DATA_SIZE;
        let end   = (start + CHUNK_DATA_SIZE).min(512);

        let mut chunk = [0u8; 2 + CHUNK_DATA_SIZE];
        chunk[..2].copy_from_slice(&encode_u16(i as u16));
        let len = end - start;
        chunk[2..2 + len].copy_from_slice(&data[start..end]);

        let r = syscall::send(DISK_SERVER_PID, MSG_WRITE_CHUNK, &chunk[..2 + len]);
        if r == usize::MAX {
            return Err("disk: failed to send chunk");
        }
    }

    // Tunggu konfirmasi
    let mut msg = Message::empty();
    syscall::recv(&mut msg);

    match msg.kind {
        MSG_DONE  => Ok(()),
        MSG_ERROR => Err("disk: write failed"),
        _         => Err("disk: unexpected response"),
    }
}
