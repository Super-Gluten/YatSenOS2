#![no_std]

#[macro_use]
extern crate log;
extern crate alloc;

use core::ptr::{copy_nonoverlapping, write_bytes};

use x86_64::structures::paging::page::{PageRange, PageRangeInclusive};
use x86_64::structures::paging::{mapper::*, *};
use x86_64::{PhysAddr, VirtAddr, align_up};
use xmas_elf::{ElfFile, program};
use alloc::vec::Vec;

/// Map physical memory
///
/// map [0, max_addr) to virtual space [offset, offset + max_addr)
pub fn map_physical_memory(
    offset: u64,
    max_addr: u64,
    page_table: &mut impl Mapper<Size2MiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    trace!("Mapping physical memory...");
    let start_frame = PhysFrame::containing_address(PhysAddr::new(0));
    let end_frame = PhysFrame::containing_address(PhysAddr::new(max_addr));

    for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
        let page = Page::containing_address(VirtAddr::new(frame.start_address().as_u64() + offset));
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            page_table
                .map_to(page, frame, flags, frame_allocator)
                .expect("Failed to map physical memory")
                .flush();
        }
    }
}

/// Map a range of memory
///
/// allocate frames and map to specified address (R/W)
pub fn map_range(
    addr: u64,
    count: u64,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    user_access: bool,
) -> Result<PageRange, MapToError<Size4KiB>> {
    let range_start = Page::containing_address(VirtAddr::new(addr));
    let range_end = range_start + count;

    trace!(
        "Page Range: {:?}({})",
        Page::range(range_start, range_end),
        count
    );

    // default flags for stack
    let mut flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

    if user_access {
        flags.insert(PageTableFlags::USER_ACCESSIBLE);
    } // 不添加这一项，将会导致shell因为缺少这一项而无法运行！
    
    for page in Page::range(range_start, range_end) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        unsafe {
            page_table
                .map_to(page, frame, flags, frame_allocator)?
                .flush();
        }
    }

    trace!(
        "Map hint: {:#x} -> {:#x}",
        addr,
        page_table
            .translate_page(range_start)
            .unwrap()
            .start_address()
    );

    Ok(Page::range(range_start, range_end))
}

/// Load & Map ELF file
///
/// load segments in ELF file to new frames and set page table
pub fn load_elf(
    elf: &ElfFile,
    physical_offset: u64,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    user_access: bool, // 0x04 add
) -> Result<Vec<PageRangeInclusive>, MapToError<Size4KiB>> {
    trace!("Loading ELF file...{:?}", elf.input.as_ptr());

    // use iterator and functional programming to load segments
    // and collect the loaded pages into a vector
    elf.program_iter()
        .filter(|segment| segment.get_type().unwrap() == program::Type::Load)
        .map(|segment| {
            load_segment(
                elf,
                physical_offset,
                &segment,
                page_table,
                frame_allocator,
                user_access,
            )
        })
        .collect()
}

/// Load & Map ELF segment
///
/// load segment to new frame and set page table
fn load_segment(
    elf: &ElfFile,
    physical_offset: u64,
    segment: &program::ProgramHeader,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    user_access: bool // 0x04 add: 用于判断是否需要添加USER_ACCESSIBLE标志
) -> Result<PageRangeInclusive, MapToError<Size4KiB>> {
    trace!("Loading & mapping segment: {:#x?}", segment);

    let mem_size = segment.mem_size();
    let file_size = segment.file_size();
    let file_offset = segment.offset() & !0xfff;
    let virt_start_addr = VirtAddr::new(segment.virtual_addr());

    // 初始化页表标志，包含PRESENT表示该页表项有效
    let mut page_table_flags = PageTableFlags::PRESENT;

    // FIXME: handle page table flags with segment flags
    // unimplemented!("Handle page table flags with segment flags!");
    // 关于PageTableFlags的相关信息可在 https://os.phil-opp.com/zh-CN/paging-introduction/#di-zhi-zhuan-huan-fan-li 中找到
    // 处理写权限，ELF 可写 -> 页表WRITALBE标志 -> 页表可写
    if segment.flags().is_write() {
        page_table_flags |= PageTableFlags::WRITABLE;
    }

    // ELF 不可执行 -> 页表NO_EXECUTE标志 -> 页表禁用执行
    if !segment.flags().is_execute() {
        page_table_flags |= PageTableFlags::NO_EXECUTE
    }

    // // 默认设置允许用户空间访问
    // page_table_flags |= PageTableFlags::USER_ACCESSIBLE;

    // 0x04 add: 根据load_elf的参数决定页表是否添加USER_ACCESSIBLE标志位
    if user_access {
        page_table_flags |= PageTableFlags::USER_ACCESSIBLE;
    }

    trace!("Segment page table flag: {:?}", page_table_flags);

    let start_page = Page::containing_address(virt_start_addr);
    let end_page = Page::containing_address(virt_start_addr + file_size - 1u64);
    let pages = Page::range_inclusive(start_page, end_page);

    let data = unsafe { elf.input.as_ptr().add(file_offset as usize) };

    for (idx, page) in pages.enumerate() {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let offset = idx as u64 * page.size();
        let count = if file_size - offset < page.size() {
            file_size - offset
        } else {
            page.size()
        };

        unsafe {
            copy_nonoverlapping(
                data.add(idx * page.size() as usize),
                (frame.start_address().as_u64() + physical_offset) as *mut u8,
                count as usize,
            );

            page_table
                .map_to(page, frame, page_table_flags, frame_allocator)?
                .flush();

            if count < page.size() {
                // zero the rest of the page
                trace!(
                    "Zeroing rest of the page: {:#x}",
                    page.start_address().as_u64()
                );
                write_bytes(
                    (frame.start_address().as_u64() + physical_offset + count) as *mut u8,
                    0,
                    (page.size() - count) as usize,
                );
            }
        }
    }

    if mem_size > file_size {
        // .bss section (or similar), which needs to be zeroed
        let zero_start = virt_start_addr + file_size;
        let zero_end = virt_start_addr + mem_size;

        // Map additional frames.
        let start_address = VirtAddr::new(align_up(zero_start.as_u64(), Size4KiB::SIZE));
        let start_page: Page = Page::containing_address(start_address);
        let end_page = Page::containing_address(zero_end);

        for page in Page::range_inclusive(start_page, end_page) {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(MapToError::FrameAllocationFailed)?;

            unsafe {
                page_table
                    .map_to(page, frame, page_table_flags, frame_allocator)?
                    .flush();

                // zero bss section
                write_bytes(
                    (frame.start_address().as_u64() + physical_offset) as *mut u8,
                    0,
                    page.size() as usize,
                );
            }
        }
    }

    Ok(Page::range_inclusive(start_page, end_page))
}

// 0x04 add 自定义函数：用于创建用户态堆且不改变原有map_range
/// Map a range of memory for user heap
///
/// allocate frames and map to specified address (R/W)
pub fn user_map_range(
    addr: u64,
    count: u64,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<PageRange, MapToError<Size4KiB>> {
    let range_start = Page::containing_address(VirtAddr::new(addr));
    let range_end = range_start + count;

    trace!(
        "Page Range: {:?}({})",
        Page::range(range_start, range_end),
        count
    );

    let flags = PageTableFlags::PRESENT 
        | PageTableFlags::WRITABLE
        | PageTableFlags::USER_ACCESSIBLE
        | PageTableFlags::NO_EXECUTE;

    for page in Page::range(range_start, range_end) {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        unsafe {
            page_table
                .map_to(page, frame, flags, frame_allocator)?
                .flush();
        }
    }

    trace!(
        "Map hint: {:#x} -> {:#x}",
        addr,
        page_table
            .translate_page(range_start)
            .unwrap()
            .start_address()
    );

    Ok(Page::range(range_start, range_end))
}

// 0x07 add: ummap_range函数和unmap_page函数
/// Unmap a range of memory
///
/// deallocate frames and unmap to specified address (R/W)
pub fn unmap_pages(
    addr: u64,
    pages: u64,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_deallocator: &mut impl FrameDeallocator<Size4KiB>,
    do_dealloc: bool,
) -> Result<(), UnmapError> {
    debug_assert!(pages > 0, "pages must be greater than 0");
    let start = Page::containing_address(VirtAddr::new(addr));
    let end = start + pages - 1;

    unmap_range(
        Page::range_inclusive(start, end),
        page_table,
        frame_deallocator,
        do_dealloc,
    )
}

pub fn unmap_range(
    page_range: PageRangeInclusive,
    page_table: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameDeallocator<Size4KiB>,
    dealloc: bool,
) -> Result<(), UnmapError> {
    trace!(
        "Unmap Range: {:#} - {:#} ({})",
        page_range.start.start_address().as_u64(),
        page_range.end.start_address().as_u64(),
        page_range.count()
    );

    for page in page_range {
        let (frame ,flush) = page_table.unmap(page)?;
        if dealloc {
            unsafe {
                frame_allocator.deallocate_frame(frame);
            }
        }
        flush.flush();
    }
    Ok(())
}