//! Physical frame allocator and 4 KiB page mapping on top of the boot page
//! tables. Physical RAM below 1 GiB is identity-mapped, so page-table frames
//! can be touched directly by their physical address.

use crate::memory;
use alloc::vec::Vec;
use spin::Mutex;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    mapper::MapToError, FrameAllocator, FrameDeallocator, Mapper, OffsetPageTable, Page,
    PageTable, PageTableFlags, PhysFrame, Size4KiB, Translate,
};
use x86_64::{PhysAddr, VirtAddr};

const MAX_REGIONS: usize = 16;

struct Frames {
    regions: [(u64, u64); MAX_REGIONS], // (start, end), page aligned
    count: usize,
    idx: usize,
    next: u64,
    free: Vec<u64>,
    allocated: usize,
}

static FRAMES: Mutex<Option<Frames>> = Mutex::new(None);

/// Starts frame allocation from usable RAM at or above `floor` (below the
/// identity-mapped limit). Returns the number of frames available.
///
/// # Safety
/// `info` must be the multiboot2 pointer; `floor` must be above everything
/// already in use (kernel, multiboot info, heap). Heap must be initialised.
pub unsafe fn init_frames(info: usize, floor: u64) -> usize {
    let mut regions = [(0u64, 0u64); MAX_REGIONS];
    let mut count = 0;
    let mut total = 0usize;
    memory::for_each_usable(info, |r| {
        let start = memory::align_up(r.base.max(floor), 4096);
        let end = r.end().min(memory::IDENTITY_LIMIT) & !4095;
        if end > start && count < MAX_REGIONS {
            regions[count] = (start, end);
            count += 1;
            total += ((end - start) / 4096) as usize;
        }
    });
    let next = if count > 0 { regions[0].0 } else { 0 };
    *FRAMES.lock() = Some(Frames {
        regions,
        count,
        idx: 0,
        next,
        free: Vec::new(),
        allocated: 0,
    });
    total
}

/// Number of frames currently handed out.
pub fn frames_in_use() -> usize {
    FRAMES.lock().as_ref().map_or(0, |f| f.allocated)
}

pub struct KernelFrames;

unsafe impl FrameAllocator<Size4KiB> for KernelFrames {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let mut guard = FRAMES.lock();
        let f = guard.as_mut()?;
        let addr = if let Some(a) = f.free.pop() {
            a
        } else {
            loop {
                if f.idx >= f.count {
                    return None;
                }
                let (_, end) = f.regions[f.idx];
                if f.next + 4096 <= end {
                    let a = f.next;
                    f.next += 4096;
                    break a;
                }
                f.idx += 1;
                if f.idx < f.count {
                    f.next = f.regions[f.idx].0;
                }
            }
        };
        f.allocated += 1;
        // Hand out zeroed frames (identity mapped).
        unsafe { core::ptr::write_bytes(addr as *mut u8, 0, 4096) };
        Some(PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

impl FrameDeallocator<Size4KiB> for KernelFrames {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame) {
        if let Some(f) = FRAMES.lock().as_mut() {
            f.free.push(frame.start_address().as_u64());
            f.allocated -= 1;
        }
    }
}

/// Mapper over the currently active page tables (physical offset 0).
unsafe fn mapper() -> OffsetPageTable<'static> {
    let (l4, _) = Cr3::read();
    let table = &mut *(l4.start_address().as_u64() as *mut PageTable);
    OffsetPageTable::new(table, VirtAddr::new(0))
}

/// Maps `virt` to a freshly allocated frame.
pub fn map_new_page(virt: u64, flags: PageTableFlags) -> Result<PhysAddr, MapToError<Size4KiB>> {
    let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt));
    let mut alloc = KernelFrames;
    let frame = alloc.allocate_frame().ok_or(MapToError::FrameAllocationFailed)?;
    let flags = flags | PageTableFlags::PRESENT;
    unsafe {
        match mapper().map_to(page, frame, flags, &mut alloc) {
            Ok(flush) => flush.flush(),
            Err(e) => {
                alloc.deallocate_frame(frame);
                return Err(e);
            }
        }
    }
    Ok(frame.start_address())
}

/// Unmaps `virt` and returns its frame to the allocator.
pub fn unmap_page(virt: u64) -> bool {
    let page = Page::<Size4KiB>::containing_address(VirtAddr::new(virt));
    unsafe {
        match mapper().unmap(page) {
            Ok((frame, flush)) => {
                flush.flush();
                KernelFrames.deallocate_frame(frame);
                true
            }
            Err(_) => false,
        }
    }
}

/// Virtual → physical translation using the live page tables.
pub fn translate(virt: u64) -> Option<PhysAddr> {
    unsafe { mapper().translate_addr(VirtAddr::new(virt)) }
}
