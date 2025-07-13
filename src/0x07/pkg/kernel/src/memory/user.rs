use crate::proc::PageTableContext;
use linked_list_allocator::LockedHeap;
use x86_64::VirtAddr;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB, mapper::MapToError,
};

pub const USER_HEAP_START: usize = 0x4000_0000_0000;
pub const USER_HEAP_SIZE: usize = 1024 * 1024; // 1 MiB
const USER_HEAP_PAGE: usize = USER_HEAP_SIZE / crate::memory::PAGE_SIZE as usize;

pub static USER_ALLOCATOR: LockedHeap = LockedHeap::empty();

// NOTE: export mod user / call in the kernel init / after frame allocator
pub fn init() {
    init_user_heap().expect("User Heap Initialization Failed.");
    info!("User Heap Initialized.");
}

pub fn init_user_heap() -> Result<(), MapToError<Size4KiB>> {
    // Get current pagetable mapper
    let mapper = &mut PageTableContext::new().mapper();
    // Get global frame allocator
    let frame_allocator = &mut *super::get_frame_alloc_for_sure();

    // FIXME: use elf::map_range to allocate & map
    //        frames (R/W/User Access)

    let user_heap_start = VirtAddr::new(USER_HEAP_START as u64);
    let user_heap_end = user_heap_start + USER_HEAP_SIZE as u64;

    elf::user_map_range(
        USER_HEAP_START as u64,
        USER_HEAP_PAGE as u64,
        mapper,
        frame_allocator,
    );

    unsafe {
        USER_ALLOCATOR
            .lock()
            .init(USER_HEAP_START as *mut u8, USER_HEAP_SIZE);
    }

    debug!(
        "User Heap      : 0x{:016x}-0x{:016x}",
        user_heap_start.as_u64(),
        user_heap_end.as_u64()
    );

    let (size, unit) = crate::humanized_size(USER_HEAP_SIZE as u64);
    info!("User Heap Size : {:>7.*} {}", 3, size, unit);

    info!("User Heap Initialized.");

    Ok(())
}
// 根据 elf/lib.rs map_range()
// 新定义了一个user_map_range() in elf/lib.rs；
// 整体init_user_heap() 结构仿照memory/allocator.rs中的init()
