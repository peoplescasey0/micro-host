//! Block storage interface, MBR partition table discovery, and asynchronous I/O job spooler.
//!
//! Ported from:
//! - `integrations/hypervisor/bitvisor-3.0/include/storage.h` (`StorageAccess`, LBA sector I/O)
//! - `integrations/hypervisor/cosmos-os` (MBR partition table discovery & validation)
//! - Post Card OS required subsystems (`README.md`: Partitioning, Buffering, Spooling)

use crate::sync::Mutex;
use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub const SECTOR_SIZE: usize = 512;
pub const LBA_INVALID: u64 = u64::MAX;
pub const MAX_SPOOL_JOBS: usize = 32;

/// BitVisor storage direction (`STORAGE_READ` / `STORAGE_WRITE` in `storage.h`).
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageRw {
    Read = 0,
    Write = 1,
}

/// BitVisor sector access descriptor (`struct storage_access` in `storage.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageAccess {
    pub lba: u64,
    pub count: u32,
    pub sector_size: usize,
    pub rw: StorageRw,
}

/// Standard MBR Partition Entry (16 bytes at offset `0x1BE + index * 16`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Parse and validate an MBR sector (512 bytes) and discover non-empty partitions
/// within `total_sectors` boundaries.
pub fn discover_mbr_partitions(
    mbr_sector: &[u8; SECTOR_SIZE],
    total_sectors: u64,
) -> Result<Vec<MbrPartition>, &'static str> {
    if mbr_sector[510] != 0x55 || mbr_sector[511] != 0xAA {
        return Err("Invalid MBR boot signature (expected 0x55AA)");
    }
    let mut partitions = Vec::new();
    for idx in 0..4u8 {
        let offset = 0x1BE + (idx as usize) * 16;
        let status = mbr_sector[offset];
        let ptype = mbr_sector[offset + 4];
        let start_lba = u32::from_le_bytes([
            mbr_sector[offset + 8],
            mbr_sector[offset + 9],
            mbr_sector[offset + 10],
            mbr_sector[offset + 11],
        ]);
        let count = u32::from_le_bytes([
            mbr_sector[offset + 12],
            mbr_sector[offset + 13],
            mbr_sector[offset + 14],
            mbr_sector[offset + 15],
        ]);
        if ptype == 0 || count == 0 {
            continue;
        }
        let end_lba = (start_lba as u64) + (count as u64);
        if start_lba == 0 || end_lba > total_sectors {
            return Err("Partition exceeds disk LBA boundaries");
        }
        partitions.push(MbrPartition {
            index: idx + 1,
            bootable: status == 0x80,
            partition_type: ptype,
            start_lba,
            sector_count: count,
        });
    }
    Ok(partitions)
}

/// Synthesize a valid 3-partition MBR sector for Post Card OS:
/// - `P1` (`0x83`, bootable): Root OS / Kernel + Ramdisk partition
/// - `P2` (`0x0C`, FAT32): Immutable `CONFIG` partition (harvested from `#1`/`#2` layout)
/// - `P3` (`0x83`, ext4): Persistent Card Overlay & Spool partition
pub fn build_default_mbr(total_sectors: u32) -> [u8; SECTOR_SIZE] {
    let mut mbr = [0u8; SECTOR_SIZE];
    let p1_start: u32 = 2048;
    let config_sectors: u32 = 2048; // 1 MiB CONFIG partition
    let persist_sectors: u32 = 65536; // 32 MiB PERSIST/SPOOL partition
    let p1_count: u32 = total_sectors
        .saturating_sub(p1_start + config_sectors + persist_sectors);
    let p2_start: u32 = p1_start + p1_count;
    let p3_start: u32 = p2_start + config_sectors;

    let entries = [
        (0x80u8, 0x83u8, p1_start, p1_count),
        (0x00u8, 0x0Cu8, p2_start, config_sectors),
        (0x00u8, 0x83u8, p3_start, persist_sectors),
    ];

    for (i, (status, ptype, start, count)) in entries.iter().enumerate() {
        if *count == 0 {
            continue;
        }
        let off = 0x1BE + i * 16;
        mbr[off] = *status;
        mbr[off + 4] = *ptype;
        mbr[off + 8..off + 12].copy_from_slice(&start.to_le_bytes());
        mbr[off + 12..off + 16].copy_from_slice(&count.to_le_bytes());
    }
    mbr[510] = 0x55;
    mbr[511] = 0xAA;
    mbr
}

/// Spooler job status (`README.md` Spooling requirement).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpoolJobState {
    Queued,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone)]
pub struct SpoolJob {
    pub id: u64,
    pub device: String,
    pub access: StorageAccess,
    pub payload: Vec<u8>,
    pub state: SpoolJobState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpoolError {
    QueueFull,
    InvalidAccess,
    JobNotFound,
}

struct Spooler {
    next_id: u64,
    pending: VecDeque<SpoolJob>,
    completed: Vec<SpoolJob>,
}

static SPOOLER: Mutex<Spooler> = Mutex::new(Spooler {
    next_id: 1,
    pending: VecDeque::new(),
    completed: Vec::new(),
});

/// Submit a non-blocking I/O work item to the kernel spooler with backpressure check.
pub fn submit_spool_job(
    device: &str,
    access: StorageAccess,
    payload: Vec<u8>,
) -> Result<u64, SpoolError> {
    if access.lba == LBA_INVALID || access.count == 0 || access.sector_size == 0 {
        return Err(SpoolError::InvalidAccess);
    }
    let mut spool = SPOOLER.lock();
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

/// Cancel a queued spool job before it executes.
pub fn cancel_spool_job(job_id: u64) -> Result<(), SpoolError> {
    let mut spool = SPOOLER.lock();
    let Some(pos) = spool.pending.iter().position(|j| j.id == job_id) else {
        return Err(SpoolError::JobNotFound);
    };
    let mut job = spool.pending.remove(pos).unwrap();
    job.state = SpoolJobState::Cancelled;
    spool.completed.push(job);
    Ok(())
}

/// Process the next queued I/O job in FIFO order.
pub fn process_next_spool_job() -> Option<SpoolJob> {
    let mut spool = SPOOLER.lock();
    let mut job = spool.pending.pop_front()?;
    job.state = SpoolJobState::Completed;
    let clone = job.clone();
    if spool.completed.len() >= 32 {
        spool.completed.remove(0);
    }
    spool.completed.push(job);
    Some(clone)
}

/// Return `(queued_count, completed_count)`.
pub fn spool_stats() -> (usize, usize) {
    let spool = SPOOLER.lock();
    (spool.pending.len(), spool.completed.len())
}
