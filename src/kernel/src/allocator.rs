//! Kernel heap allocation.

use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

/// The start address of the kernel heap.
pub const HEAP_START: usize = 0x_4444_4444_0000;
/// The size of the kernel heap.
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Initialize the kernel heap.
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        // SAFETY: We are mapping freshly allocated physical frames to virtual pages
        // in the heap region. The frame allocator guarantees these frames are unused.
        // The virtual address range [HEAP_START, HEAP_START + HEAP_SIZE) is reserved
        // for the kernel heap and not used elsewhere.
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    // SAFETY: The heap memory region has just been mapped above with read/write
    // permissions. HEAP_START and HEAP_SIZE define a valid, properly aligned
    // memory region. This function is only called once during kernel initialization.
    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    Ok(())
}
