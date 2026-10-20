//! Heap allocator implementation for Post Card OS
//! 
//! This module provides a simple bump allocator that will later be replaced
//! with a more sophisticated allocator like buddy or slab.

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags,
        Size4KiB,
    },
    VirtAddr,
};
use linked_list_allocator::LockedHeap;
use crate::memory::BootInfoFrameAllocator;

/// Global heap allocator instance
#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Initialize the kernel heap
/// 
/// # Safety
/// This function is unsafe because it maps heap pages to physical frames.
/// The caller must ensure that the heap region doesn't overlap with other
/// memory mappings.
pub unsafe fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    heap_start: VirtAddr,
    heap_size: usize,
) -> Result<(), MapToError<Size4KiB>> {
    let heap_end = heap_start + heap_size as u64;
    let heap_start_page = Page::containing_address(heap_start);
    let heap_end_page = Page::containing_address(heap_end - 1u64);
    
    // Map each heap page to a physical frame
    for page in Page::range_inclusive(heap_start_page, heap_end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        
        mapper.map_to(page, frame, flags, frame_allocator)?.flush();
    }
    
    // Initialize the heap allocator
    HEAP_ALLOCATOR.lock().init(heap_start.as_mut_ptr(), heap_size);
    
    Ok(())
}

/// Simple bump allocator for early kernel initialization
/// 
/// This is a temporary allocator that will be replaced by the global
/// heap allocator once paging is initialized.
pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    /// Create a new bump allocator for the given memory region
    /// 
    /// # Safety
    /// The memory region must be valid and not used by anything else.
    pub const unsafe fn new(heap_start: usize, heap_size: usize) -> Self {
        BumpAllocator {
            heap_start,
            heap_end: heap_start + heap_size,
            next: heap_start,
            allocations: 0,
        }
    }
    
    /// Allocate memory with the given alignment
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<*mut u8> {
        let alloc_start = align_up(self.next, align);
        let alloc_end = alloc_start.checked_add(size)?;
        
        if alloc_end > self.heap_end {
            return None;
        }
        
        self.next = alloc_end;
        self.allocations += 1;
        
        Some(alloc_start as *mut u8)
    }
    
    /// Get the number of active allocations
    pub fn allocations(&self) -> usize {
        self.allocations
    }
    
    /// Get the total allocated size
    pub fn allocated_size(&self) -> usize {
        self.next - self.heap_start
    }
}

/// Align `addr` upwards to alignment `align`
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Test that the heap allocator works correctly
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_bump_allocator() {
        const HEAP_SIZE: usize = 1024; // 1KB test heap
        let heap = [0u8; HEAP_SIZE];
        
        unsafe {
            let mut allocator = BumpAllocator::new(heap.as_ptr() as usize, HEAP_SIZE);
            
            // Allocate some memory
            let ptr1 = allocator.allocate(16, 8);
            assert!(ptr1.is_some());
            assert_eq!(allocator.allocations(), 1);
            
            // Allocate more memory
            let ptr2 = allocator.allocate(32, 16);
            assert!(ptr2.is_some());
            assert_eq!(allocator.allocations(), 2);
            
            // Allocation should fail if out of memory
            let ptr3 = allocator.allocate(HEAP_SIZE, 8);
            assert!(ptr3.is_none());
        }
    }
    
    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 8), 0);
        assert_eq!(align_up(1, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(9, 8), 16);
        assert_eq!(align_up(15, 8), 16);
        assert_eq!(align_up(16, 8), 16);
    }
}