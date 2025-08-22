//! 内存页管理模块
//!
//! 使用 `x86_64` crate 提供的分页结构体：
//! - [`Page`]：单个内存页（4KB）
//! - [`PageRange`]：连续的页范围
//!
//! # 主要功能
//! - **页对齐操作**：所有地址必须为 4096 的倍数（`Size4KiB`）。
//! - **动态栈管理**：支持向下增长的栈（高地址 → 低地址）。
//!
//! ## 核心函数说明
//! ### `Page` 相关操作
//! | 函数/方法                         | 作用                                                                 |
//! |--------------------------------- |----------------------------------------------------------------------|
//! | `Page::containing_address(addr)` | 返回包含 `addr` 的页（自动对齐到 4KB 边界）                             |
//! | `Page::start_address()`          | 获取该页的起始物理地址（`VirtAddr` 类型）                               |
//! | `Page::<Size4KiB>::from_start_address(addr)` | 从对齐的地址构造 `Page`（需显式指定页大小）                  |
//! | `Page + usize` / `Page - usize` | 页地址算术运算（按页大小跳转，如 `Page(0x1000) + 2 = Page(0x3000)`）      |
//!
//! ### `PageRange` 相关操作
//! | 函数/方法                     | 作用                                                                 |
//! |-------------------------------|---------------------------------------------------------------------|
//! | `Page::range(start, end)`     | 构造左闭右开区间 `[start, end)` 的页范围                              |
//! | `PageRange::contains(page)`   | 检查某页是否在范围内                                                 |
//! | `PageRange::overlaps(other)`  | 检查两个页范围是否重叠                                               |
//!
//! ## 示例代码
//! ```rust
//! use x86_64::{VirtAddr, structures::paging::{Page, PageRange}};
//!
//! // 1. 创建页和页范围
//! let page = Page::containing_address(VirtAddr::new(0x3000)); // 包含地址 0x3000 的页
//! let range = Page::range(Page::from_start_address(VirtAddr::new(0x1000)),
//!                         Page::from_start_address(VirtAddr::new(0x4000))); // [0x1000, 0x4000)
//!
//! // 2. 检查页是否在范围内
//! assert!(range.contains(page)); // 0x3000 ∈ [0x1000, 0x4000)
//!
//! // 3. 页地址算术
//! let next_page = page + 1; // Page(0x4000)
//! ```
//!
//! ## 注意事项（Attention）
//! 1. **地址对齐**：所有操作必须保证地址是 4096 的倍数，否则会触发未定义行为。
//! 2. **栈增长方向**：本模块默认栈向下增长（高地址 → 低地址），`Page::range` 的 `start` 应为栈底。
//! 3. **页大小**：使用 `Size4KiB` 作为默认页大小，其他大小需显式指定（如 `Page::<Size2MiB>`）。
//!
//! 更多细节参考官方文档：
//! - [`x86_64::structures::paging::Page`](https://docs.rs/x86_64/latest/x86_64/structures/paging/struct.Page.html)
//! - [`PageRange`](https://docs.rs/x86_64/latest/x86_64/structures/paging/struct.PageRange.html)

use alloc::format;
use x86_64::{
    VirtAddr,
    structures::paging::{page::*, *},
};
use xmas_elf::ElfFile;

use crate::{humanized_size, memory::*};

pub mod stack;

use self::stack::*;

use super::{PageTableContext, ProcessId};

type MapperRef<'a> = &'a mut OffsetPageTable<'static>;
type FrameAllocatorRef<'a> = &'a mut BootInfoFrameAllocator;

pub struct ProcessVm {
    // page table is shared by parent and child
    pub(super) page_table: PageTableContext, // use struct define on paging.rs

    // stack is pre-process allocated
    pub(super) stack: Stack, // use struct define on stack.rs
}

impl ProcessVm {
    pub fn new(page_table: PageTableContext) -> Self {
        Self {
            page_table,
            stack: Stack::empty(),
        }
    }

    pub fn init_kernel_vm(mut self) -> Self {
        // use fn: kstack() and record kernel code usage
        self.stack = Stack::kstack();
        // info!("{}", self.stack.memory_usage());
        self
    }

    // Initializes the process stack for the given ProcessId
    pub fn init_proc_stack(&mut self, pid: ProcessId) -> VirtAddr {
        // calculate the stack for pid
        //
        // 1. Calculate the physical address of stack
        let stack_top_addr = STACK_INIT_TOP - STACK_MAX_SIZE * (pid.0 as u64 - 1);
        let stack_bot_addr = STACK_INIT_BOT - STACK_MAX_SIZE * (pid.0 as u64 - 1);
        info!("init stack for process with Top: [{:#x}]; Bot [{:#x}]", stack_top_addr, stack_bot_addr);

        // 2. Virtualize the stack_top and create the stack
        let virtual_stack_top_addr = VirtAddr::new(stack_top_addr);

        self.stack = Stack::new(
            Page::containing_address(virtual_stack_top_addr),
            STACK_DEF_PAGE,
        );

        // 3. Use user_map_range to perform memory mapping on the stack
        //
        // # Attention:
        // The stack grow downwards in the kernel, so the user_map_range use stack_bot_addr
        // because it's smaller than stack_top_addr
        let page_table = &mut self.page_table.mapper();
        let frame_alloc = &mut *get_frame_alloc_for_sure();
        elf::user_map_range(stack_bot_addr, STACK_DEF_PAGE, page_table, frame_alloc).unwrap();

        // 4. Return the VirtAddr at the top of stack
        virtual_stack_top_addr
    }

    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> bool {
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        self.stack.handle_page_fault(addr, mapper, alloc)
    }

    pub(super) fn memory_usage(&self) -> u64 {
        self.stack.memory_usage()
    }

    pub fn load_elf(&mut self, elf: &ElfFile) {
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        // load elf to process pagetable
        elf::load_elf(elf, *PHYSICAL_OFFSET.get().unwrap(), mapper, alloc, true).unwrap();

        self.stack.init(mapper, alloc);
    }

    pub fn clean_up_stack(&self) {
        let page_table = &mut self.page_table.mapper();
        let frame_allocator = &mut *get_frame_alloc_for_sure();

        elf::unmap_range(
            self.stack.stack_start().as_u64(),
            self.stack.stack_usage(),
            page_table,
            frame_allocator,
        )
        .expect("Failed to clean up current process' stack");
    }

    pub fn fork(&self, stack_offset_count: u64) -> Self {
        // clone the page table context (see instructions)
        let owned_page_table = self.page_table.fork();

        let mapper = &mut owned_page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        Self {
            page_table: owned_page_table,
            stack: self.stack.fork(mapper, alloc, stack_offset_count),
        }
    }
}

impl core::fmt::Debug for ProcessVm {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let (size, unit) = humanized_size(self.memory_usage());

        f.debug_struct("ProcessVm")
            .field("stack", &self.stack)
            .field("memory_usage", &format!("{} {}", size, unit))
            .field("page_table", &self.page_table)
            .finish()
    }
}
