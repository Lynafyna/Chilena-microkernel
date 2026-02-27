//! proc — process management
//!
//! Berisi semua komponen yang berhubungan dengan manajemen proses:
//!   - Process: load CHN, spawn, exec, context
//!   - Sched: scheduler, context switch

pub mod process;
pub mod sched;
