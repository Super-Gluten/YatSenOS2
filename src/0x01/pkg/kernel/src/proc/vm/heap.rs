use core::sync::atomic::{AtomicU64, Ordering};

use alloc::sync::Arc;
use x86_64::{
    structures::paging::{mapper::UnmapError, Page, Size4KiB},
    VirtAddr,
};

use super::{FrameAllocatorRef, MapperRef};

use crate::proc::{processor ,KERNEL_PID};

// user process runtime heap
// 0x100000000 bytes -> 4GiB
// from 0x0000_2000_0000_0000 to 0x0000_2000_ffff_fff8
pub const HEAP_START: u64 = 0x2000_0000_0000;
pub const HEAP_PAGES: u64 = 0x100000;
pub const HEAP_SIZE: u64 = HEAP_PAGES * crate::memory::PAGE_SIZE;
pub const HEAP_END: u64 = HEAP_START + HEAP_SIZE - 8;

/// User process runtime heap
///
/// always page aligned, the range is [base, end)
pub struct Heap {
    /// the base address of the heap
    ///
    /// immutable after initialization
    base: VirtAddr,

    /// the current end address of the heap
    ///
    /// use atomic to allow multiple threads to access the heap
    end: Arc<AtomicU64>,
}

impl Heap {
    pub fn empty() -> Self {
        Self {
            base: VirtAddr::new(HEAP_START),
            end: Arc::new(AtomicU64::new(HEAP_START)),
        }
    }

    pub fn fork(&self) -> Self {
        Self {
            base: self.base,
            end: self.end.clone(),
        }
    }

    pub fn brk(
        &self,
        new_end: Option<VirtAddr>,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> Option<VirtAddr> {
        // FIXME: if new_end is None, return the current end address
        if new_end.is_none() {
            return Some(VirtAddr::new(self.end.load(Ordering::Relaxed)));
        }

        // FIXME: check if the new_end is valid (in range [base, base + HEAP_SIZE])
        let new_end = new_end.unwrap();
        if new_end.as_u64() < HEAP_START || new_end.as_u64() > HEAP_END {
            error!("Heap brk: new_end is out of heap range");
            return None;
        }
        let mut new_end_page: Page<Size4KiB> = Page::containing_address(new_end);
        if new_end != self.base {
            new_end_page += 1;
        } // 如果不是第一次初始化，那么应该在现有基础上Page++

        // FIXME: calculate the difference between the current end and the new end
        let user_access = processor::get_pid() != KERNEL_PID; // 非内核进程，都是用户进程
        let current_end = self.end.load(Ordering::Relaxed);
        let mut current_end_page: Page<Size4KiB> = Page::containing_address(VirtAddr::new(current_end));

        if current_end != self.base.as_u64() {
            current_end_page += 1;
        } // 同上new_end_page的定义方法

        // 计算两者差距
        let difference: i64 = new_end.as_u64() as i64 - current_end as i64;
        
        // NOTE: print the heap difference for debugging
        debug!(
            "Heap difference: {:#x}, heap end addr: {:#x} -> {:#x}", 
            difference.abs() as u64, 
            current_end, 
            new_end.as_u64()
        );

        debug!(
            "Heap end page: {:#x} -> {:#x}",
            current_end_page.start_address().as_u64(),
            new_end_page.start_address().as_u64(),
        );
        // FIXME: do the actual mapping or unmapping
        if difference > 0 {
            // heap[base, current_end, new_end] -> map [current_end, new_end - 1]
            let addr = current_end_page.start_address();
            let count = (new_end_page.start_address().as_u64() - addr.as_u64()) / crate::memory::PAGE_SIZE;
            match elf::map_range(addr.as_u64(), count, mapper, alloc, user_access) {
                Ok(range) => {
                    debug!(
                        "map heap ranging from {:#?} to {:#?}",
                        range.start, range.end
                    );
                }
                Err(e) => {
                    error!("Failed to map heap: {:?}", e);
                    return None;
                }
            }
        } else if difference < 0{
            // heap[base, new_end, current_end] -> unmap [new_end, current_end -1]
            let range = Page::range_inclusive(new_end_page, current_end_page);
            match elf::unmap_range(range, mapper, alloc, user_access) {
                Ok(_) => {
                    debug!(
                        "unmap heap ranging from {:#?} to {:#?}",
                        range.start, range.end
                    );
                }
                Err(e) => {
                    error!("Failed to unmap head: {:?}", e);
                    return None;
                }
            }
        } 
        // FIXME: update the end address

        self.end.store(new_end.as_u64(), Ordering::Relaxed);
        Some(new_end)
    }

    pub(super) fn clean_up(
        &self,
        mapper: MapperRef,
        dealloc: FrameAllocatorRef,
    ) -> Result<(), UnmapError> {
        if self.memory_usage() == 0 {
            return Ok(());
        }

        // FIXME: load the current end address and **reset it to base** (use `swap`)
        let end = self.end.swap(HEAP_START, Ordering::Relaxed);
        // FIXME: unmap the heap pages
        let page_start: Page<Size4KiB> = Page::containing_address(VirtAddr::new(HEAP_START));
        let page_end: Page<Size4KiB> = Page::containing_address(VirtAddr::new(end));
        let range = Page::range_inclusive(page_start, page_end);
        unsafe {
            elf::unmap_range(range, mapper, dealloc, true)?;
        }

        Ok(())
    }

    pub fn memory_usage(&self) -> u64 {
        self.end.load(Ordering::Relaxed) - self.base.as_u64()
    }
}

impl core::fmt::Debug for Heap {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Heap")
            .field("base", &format_args!("{:#x}", self.base.as_u64()))
            .field(
                "end",
                &format_args!("{:#x}", self.end.load(Ordering::Relaxed)),
            )
            .finish()
    }
}
