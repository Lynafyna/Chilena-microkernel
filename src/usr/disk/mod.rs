//! Disk subsystem untuk Chilena
//!
//! - proto:  Protokol IPC antara client dan server
//! - server: DiskServer (userspace disk driver)
//! - client: DiskClient (API untuk userspace programs)

pub mod proto;
pub mod server;
pub mod client;
