//! Multiboot2 memory map: parsed once, very early, into a kernel-owned copy.
//!
//! GRUB may place the multiboot2 info structure in memory that overlaps the
//! kernel's own data/bss, so nothing may read `info` after [`init`] returns.
//! The boot stub identity-maps the first 1 GiB, so physical addresses below
//! that limit can be dereferenced directly.

use crate::serial_println;

pub const IDENTITY_LIMIT: u64 = 1 << 30;

const TAG_END: u32 = 0;
const TAG_MMAP: u32 = 6;
const MMAP_AVAILABLE: u32 = 1;
const MAX_REGIONS: usize = 32;

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

struct Map {
    regions: [Region; MAX_REGIONS],
    count: usize,
    info: (u64, u64), // original location of the multiboot2 info, for diagnostics
}

static mut MAP: Map = Map {
    regions: [Region { base: 0, len: 0 }; MAX_REGIONS],
    count: 0,
    info: (0, 0),
};

pub fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

pub fn kernel_end_addr() -> u64 {
    core::ptr::addr_of!(kernel_end) as u64
}

/// Copies the usable-RAM entries out of the multiboot2 info. Call this first
/// thing in `_start`, before anything else touches memory.
///
/// # Safety
/// `info` must be the pointer GRUB passed in EBX and lie in identity-mapped
/// RAM. Call once.
pub unsafe fn init(info: usize) {
    // Parse into a local first so a (hypothetical) overlap between the info
    // structure and MAP cannot corrupt the parse.
    let mut local = [Region { base: 0, len: 0 }; MAX_REGIONS];
    let mut count = 0;
    let mut total = 0usize;
    if info != 0 && (info as u64) < IDENTITY_LIMIT {
        total = core::ptr::read(info as *const u32) as usize;
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
                    if kind == MMAP_AVAILABLE && count < MAX_REGIONS {
                        local[count] = Region { base, len };
                        count += 1;
                    }
                    entry += entry_size;
                }
            }
            tag += (size + 7) & !7;
        }
    }
    let map = &mut *core::ptr::addr_of_mut!(MAP);
    map.regions = local;
    map.count = count;
    map.info = (info as u64, (info + total) as u64);
}

/// Calls `f` for every available RAM region.
pub fn for_each_usable(mut f: impl FnMut(Region)) {
    let map = unsafe { &*core::ptr::addr_of!(MAP) };
    for r in &map.regions[..map.count] {
        f(*r);
    }
}

/// Where GRUB put the multiboot2 info (start, end), as recorded by [`init`].
pub fn info_range() -> (u64, u64) {
    unsafe { (*core::ptr::addr_of!(MAP)).info }
}

/// Prints the usable memory map and returns total usable bytes.
pub fn report() -> u64 {
    let mut total = 0;
    for_each_usable(|r| {
        serial_println!("  usable: {:#012x} - {:#012x} ({} KiB)", r.base, r.end(), r.len / 1024);
        total += r.len;
    });
    total
}
