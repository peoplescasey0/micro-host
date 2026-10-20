//! Multiboot2 memory map parsing.
//!
//! The boot stub identity-maps the first 1 GiB, so physical addresses below
//! that limit can be dereferenced directly.

use crate::serial_println;

pub const IDENTITY_LIMIT: u64 = 1 << 30;

const TAG_END: u32 = 0;
const TAG_MMAP: u32 = 6;
const MMAP_AVAILABLE: u32 = 1;

extern "C" {
    static kernel_end: u8;
}

#[derive(Clone, Copy)]
pub struct Region {
    pub base: u64,
    pub len: u64,
}

impl Region {
    pub fn end(&self) -> u64 {
        self.base + self.len
    }
}

pub fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

pub fn kernel_end_addr() -> u64 {
    core::ptr::addr_of!(kernel_end) as u64
}

/// End address of the multiboot2 info structure.
///
/// # Safety
/// `info` must be the pointer GRUB passed in EBX and lie in identity-mapped RAM.
pub unsafe fn info_end(info: usize) -> u64 {
    info as u64 + core::ptr::read(info as *const u32) as u64
}

/// Calls `f` for every available RAM region in the multiboot2 memory map.
///
/// # Safety
/// `info` must be the pointer GRUB passed in EBX and lie in identity-mapped RAM.
pub unsafe fn for_each_usable(info: usize, mut f: impl FnMut(Region)) {
    if info == 0 || info as u64 >= IDENTITY_LIMIT {
        return;
    }
    let total = core::ptr::read(info as *const u32) as usize;
    let mut tag = info + 8;
    let end = info + total;
    while tag + 8 <= end {
        let ty = core::ptr::read(tag as *const u32);
        let size = core::ptr::read((tag + 4) as *const u32) as usize;
        if ty == TAG_END || size < 8 {
            break;
        }
        if ty == TAG_MMAP {
            let entry_size = core::ptr::read((tag + 8) as *const u32) as usize;
            let mut entry = tag + 16;
            while entry_size >= 24 && entry + entry_size <= tag + size {
                let base = core::ptr::read(entry as *const u64);
                let len = core::ptr::read((entry + 8) as *const u64);
                let kind = core::ptr::read((entry + 16) as *const u32);
                if kind == MMAP_AVAILABLE {
                    f(Region { base, len });
                }
                entry += entry_size;
            }
        }
        tag += (size + 7) & !7;
    }
}

/// Prints the usable memory map and returns total usable bytes.
///
/// # Safety
/// See [`for_each_usable`].
pub unsafe fn report(info: usize) -> u64 {
    let mut total = 0;
    for_each_usable(info, |r| {
        serial_println!("  usable: {:#012x} - {:#012x} ({} KiB)", r.base, r.end(), r.len / 1024);
        total += r.len;
    });
    total
}
