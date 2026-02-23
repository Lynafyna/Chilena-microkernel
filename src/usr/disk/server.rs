//! DiskServer — Userspace disk driver untuk Chilena
//!
//! Jalan sebagai proses pertama setelah boot (PID 1).
//! Menerima request via IPC, akses VirtIO block melalui syscall,
//! dan balas dengan data via IPC.
//!
//! Arsitektur microkernel:
//!   Shell ──IPC──► DiskServer ──syscall──► Kernel VirtIO

use crate::api::process::ExitCode;
use crate::api::syscall;
use crate::sys::ipc::Message;
use crate::sys::virtio;
use super::proto::*;

/// Jalankan DiskServer — loop selamanya melayani request
pub fn run() -> Result<(), ExitCode> {
    klog!("DiskServer: started (PID {})", crate::sys::process::current_pid());

    if !virtio::is_available() {
        kwarn!("DiskServer: VirtIO not available, disk operations will fail");
    } else {
        klog!("DiskServer: VirtIO ready, {} sectors", virtio::capacity());
    }

    let mut sector_buf = [0u8; 512];
    let mut msg = Message::empty();

    loop {
        // Tunggu request dari siapapun
        syscall::recv(&mut msg);

        let sender = msg.sender;
        let kind   = msg.kind;

        match kind {
            // -----------------------------------------------------------------
            MSG_PING => {
                // Balas pong
                let mut reply = [0u8; 8];
                reply[..8].copy_from_slice(&encode_u64(virtio::capacity()));
                send_reply(sender, MSG_PONG, &reply);
            }

            // -----------------------------------------------------------------
            MSG_CAPACITY => {
                let cap = virtio::capacity();
                send_reply(sender, MSG_CAPACITY_REPLY, &encode_u64(cap));
            }

            // -----------------------------------------------------------------
            MSG_READ => {
                let sector = decode_u64(&msg.data[..8]);

                match virtio::read_sector(sector, &mut sector_buf) {
                    Ok(()) => {
                        // Kirim data dalam chunks
                        send_sector_chunks(sender, &sector_buf);
                    }
                    Err(e) => {
                        let err = e.as_bytes();
                        let mut data = [0u8; 64];
                        let n = err.len().min(64);
                        data[..n].copy_from_slice(&err[..n]);
                        send_reply(sender, MSG_ERROR, &data[..n]);
                    }
                }
            }

            // -----------------------------------------------------------------
            MSG_WRITE => {
                let sector = decode_u64(&msg.data[..8]);

                // Terima CHUNKS_PER_SECTOR chunk dari client
                let mut write_buf = [0u8; 512];
                let mut chunks_received = 0usize;
                let mut ok = true;

                for _ in 0..CHUNKS_PER_SECTOR {
                    let mut chunk_msg = Message::empty();
                    syscall::recv(&mut chunk_msg);

                    if chunk_msg.kind != MSG_WRITE_CHUNK {
                        ok = false;
                        break;
                    }

                    let idx   = decode_u16(&chunk_msg.data[..2]) as usize;
                    let start = idx * CHUNK_DATA_SIZE;
                    let end   = (start + CHUNK_DATA_SIZE).min(512);
                    if start < 512 {
                        let chunk_data = &chunk_msg.data[2..2 + (end - start)];
                        write_buf[start..end].copy_from_slice(chunk_data);
                    }
                    chunks_received += 1;
                }

                if ok && chunks_received == CHUNKS_PER_SECTOR {
                    match virtio::write_sector(sector, &mut write_buf) {
                        Ok(()) => send_reply(sender, MSG_DONE, &[0]),
                        Err(e) => {
                            let err = e.as_bytes();
                            let mut data = [0u8; 64];
                            let n = err.len().min(64);
                            data[..n].copy_from_slice(&err[..n]);
                            send_reply(sender, MSG_ERROR, &data[..n]);
                        }
                    }
                } else {
                    send_reply(sender, MSG_ERROR, b"write: bad chunk sequence");
                }
            }

            // -----------------------------------------------------------------
            _ => {
                // Unknown request
                send_reply(sender, MSG_ERROR, b"unknown request");
            }
        }
    }
}

/// Kirim satu sektor dalam beberapa chunk IPC messages
fn send_sector_chunks(target: usize, sector: &[u8; 512]) {
    for i in 0..CHUNKS_PER_SECTOR {
        let start = i * CHUNK_DATA_SIZE;
        let end   = (start + CHUNK_DATA_SIZE).min(512);

        let mut data = [0u8; 2 + CHUNK_DATA_SIZE];
        // 2 bytes: chunk index
        data[..2].copy_from_slice(&encode_u16(i as u16));
        // sisa: chunk data
        let chunk_len = end - start;
        data[2..2 + chunk_len].copy_from_slice(&sector[start..end]);

        crate::sys::ipc::send(target, MSG_READ_CHUNK, &data);
    }

    // Kirim DONE
    crate::sys::ipc::send(target, MSG_DONE, &[0]);
}

/// Helper: kirim reply IPC ke sender
fn send_reply(target: usize, kind: u32, data: &[u8]) {
    crate::sys::ipc::send(target, kind, data);
}
