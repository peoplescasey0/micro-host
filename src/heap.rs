//! Kernel heap, carved out of identity-mapped usable RAM.

use crate::memory::{self, Region};
use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub const HEAP_SIZE: u64 = 4 * 1024 * 1024;

/// Finds a free `HEAP_SIZE` block above the kernel image and the multiboot
/// info structure, and hands it to the allocator. Returns (start, size).
///
/// # Safety
/// Call once, after `memory::init`.
pub unsafe fn init() -> Option<(u64, u64)> {
    let floor = memory::align_up(memory::kernel_end_addr().max(1024 * 1024), 4096);

    let mut found: Option<Region> = None;
    memory::for_each_usable(|r| {
        if found.is_some() {
            return;
        }
        let start = r.base.max(floor);
        let end = r.end().min(memory::IDENTITY_LIMIT);
        if end > start && end - start >= HEAP_SIZE {
            found = Some(Region { base: start, len: HEAP_SIZE });
        }
    });

    let region = found?;
    ALLOCATOR.lock().init(region.base as *mut u8, region.len as usize);
    Some((region.base, region.len))
}
