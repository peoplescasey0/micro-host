//! Block storage interface, MBR partition table discovery, and asynchronous I/O job spooler.
//!
//! Ported from:
//! - `integrations/hypervisor/bitvisor-3.0/include/storage.h` (`StorageAccess`, LBA sector I/O)
//! - `integrations/hypervisor/cosmos-os` (MBR partition table discovery & validation)
//! - Post Card OS required subsystems (`README.md`: Partitioning, Buffering, Spooling)

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;
use thiserror::Error;

pub const SECTOR_SIZE: usize = 512;
pub const LBA_INVALID: u64 = u64::MAX;
pub const MAX_SPOOL_JOBS: usize = 32;

/// BitVisor storage direction (`STORAGE_READ` / `STORAGE_WRITE` in `storage.h`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageRw {
    Read = 0,
    Write = 1,
}

/// BitVisor sector access descriptor (`struct storage_access` in `storage.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageAccess {
    pub lba: u64,
    pub count: u32,
    pub sector_size: usize,
    pub rw: StorageRw,
}

/// Standard MBR Partition Entry (16 bytes at offset `0x1BE + index * 16`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MbrPartition {
    pub index: u8,
    pub bootable: bool,
    pub partition_type: u8,
    pub start_lba: u32,
    pub sector_count: u32,
}

impl MbrPartition {
    pub fn size_bytes(&self) -> u64 {
        (self.sector_count as u64) * (SECTOR_SIZE as u64)
    }

    pub fn type_name(&self) -> &'static str {
        match self.partition_type {
            0x83 => "Linux ext4/xfs",
            0xEF => "EFI System",
            0x07 => "NTFS/exFAT",
            0x0B | 0x0C => "FAT32",
            0x82 => "Linux Swap",
            _ => "Raw/Custom",
        }
    }
}

/// Parse MBR partition table from raw sector 0 data.
pub fn parse_mbr_partitions(sector_data: &[u8]) -> Result<Vec<MbrPartition>, String> {
    if sector_data.len() < SECTOR_SIZE {
        return Err("Sector data smaller than 512 bytes".to_string());
    }
    // Check standard 0x55AA boot signature
    if sector_data[510] != 0x55 || sector_data[511] != 0xAA {
        return Err("Invalid MBR boot signature (missing 0x55AA at offset 510)".to_string());
    }

    let mut partitions = Vec::new();
    for i in 0..4 {
        let offset = 0x1BE + i * 16;
        let entry = &sector_data[offset..offset + 16];
        let p_type = entry[4];
        if p_type == 0x00 {
            continue; // Empty slot
        }
        let bootable = entry[0] == 0x80;
        let start_lba = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]);
        let sector_count = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]);

        partitions.push(MbrPartition {
            index: (i + 1) as u8,
            bootable,
            partition_type: p_type,
            start_lba,
            sector_count,
        });
    }

    Ok(partitions)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpoolJobState {
    Queued,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpoolJob {
    pub id: u64,
    pub device: String,
    pub access: StorageAccess,
    pub payload: Vec<u8>,
    pub state: SpoolJobState,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SpoolError {
    #[error("Spool queue is full (max {MAX_SPOOL_JOBS} jobs)")]
    QueueFull,
    #[error("Job not found in pending queue")]
    JobNotFound,
    #[error("Invalid storage access parameters")]
    InvalidAccess,
}

pub struct Spooler {
    next_id: u64,
    pending: VecDeque<SpoolJob>,
    completed: Vec<SpoolJob>,
}

impl Spooler {
    pub const fn new() -> Self {
        Self {
            next_id: 1,
            pending: VecDeque::new(),
            completed: Vec::new(),
        }
    }
}

pub static SPOOLER: Mutex<Spooler> = Mutex::new(Spooler::new());

pub fn submit_spool_job(
    device: &str,
    access: StorageAccess,
    payload: Vec<u8>,
) -> Result<u64, SpoolError> {
    if access.lba == LBA_INVALID || access.count == 0 || access.sector_size == 0 {
        return Err(SpoolError::InvalidAccess);
    }
    let mut spool = SPOOLER.lock().unwrap();
    if spool.pending.len() >= MAX_SPOOL_JOBS {
        return Err(SpoolError::QueueFull);
    }
    let id = spool.next_id;
    spool.next_id += 1;
    spool.pending.push_back(SpoolJob {
        id,
        device: device.to_string(),
        access,
        payload,
        state: SpoolJobState::Queued,
    });
    Ok(id)
}

pub fn cancel_spool_job(job_id: u64) -> Result<(), SpoolError> {
    let mut spool = SPOOLER.lock().unwrap();
    let pos = spool
        .pending
        .iter()
        .position(|j| j.id == job_id)
        .ok_or(SpoolError::JobNotFound)?;
    let mut job = spool.pending.remove(pos).unwrap();
    job.state = SpoolJobState::Cancelled;
    spool.completed.push(job);
    Ok(())
}

pub fn process_next_spool_job() -> Option<SpoolJob> {
    let mut spool = SPOOLER.lock().unwrap();
    let mut job = spool.pending.pop_front()?;
    job.state = SpoolJobState::Completed;
    let clone = job.clone();
    if spool.completed.len() >= 32 {
        spool.completed.remove(0);
    }
    spool.completed.push(job);
    Some(clone)
}

pub fn spool_stats() -> (usize, usize) {
    let spool = SPOOLER.lock().unwrap();
    (spool.pending.len(), spool.completed.len())
}
