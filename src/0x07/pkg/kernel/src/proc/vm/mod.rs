use crate::{humanized_size, memory::*};
use alloc::{format, vec::Vec};
use x86_64::{
    VirtAddr,
    structures::paging::{
        mapper::{CleanUp, UnmapError},
        page::*,
        *,
    },
};
use xmas_elf::ElfFile;

pub mod heap;
pub mod stack;

use self::{heap::Heap, stack::*};
use boot::KernelPages;

use super::{PageTableContext, ProcessId};

// See the documentation for the `KernelPages` type
// Ignore when you not reach this part
//
// use boot::KernelPages;

type MapperRef<'a> = &'a mut OffsetPageTable<'static>;
type FrameAllocatorRef<'a> = &'a mut BootInfoFrameAllocator;

pub struct ProcessVm {
    // page table is shared by parent and child
    pub(super) page_table: PageTableContext, // 使用paging.rs中的结构体

    // stack is pre-process allocated
    pub(super) stack: Stack, // 使用stack.rs中的结构体

    // heap is allocated by brk syscall
    pub(super) heap: Heap,

    // code is hold by the first process
    // these fields will be empty for other processes
    pub(super) code: Vec<PageRangeInclusive>,
    pub(super) code_usage: u64,
}

impl ProcessVm {
    pub fn new(page_table: PageTableContext) -> Self {
        Self {
            page_table,
            stack: Stack::empty(),
            heap: Heap::empty(),
            code: Vec::new(),
            code_usage: 0,
        }
    }

    // / Initialize kernel vm
    // /
    // / NOTE: this function should only be called by the first process
    pub fn init_kernel_vm(mut self, pages: &KernelPages) -> Self {
        // FIXME: record kernel code usage
        self.code = pages.iter().cloned().collect();
        self.code_usage = pages.iter().map(|page| page.count() as u64).sum();

        self.stack = Stack::kstack();

        // ignore heap for kernel process as we don't manage it

        self
    }

    // pub fn init_kernel_vm(mut self) -> Self {
    //     // TODO: record kernel code usage
    //     self.stack = Stack::kstack();
    //     self
    // } // 0x07 delete

    pub fn init_proc_stack(&mut self, pid: ProcessId) -> VirtAddr {
        // FIXME: calculate the stack for pid
        info!("{:?}", STACK_INIT_TOP);
        let stack_top_addr = STACK_INIT_TOP - STACK_MAX_SIZE * (pid.0 as u64 - 1); // 计算对应用户栈栈顶地址
        let stack_bot_addr = STACK_INIT_BOT - STACK_MAX_SIZE * (pid.0 as u64 - 1);
        info!("top {:?} bot {:?}", stack_top_addr, stack_bot_addr);
        // 默认用户栈分配大小为 STACK_DEF_SIZE
        // 计算对应的栈底物理地址

        let virtual_stack_top_addr = VirtAddr::new(stack_top_addr); // 构建该进程用户栈栈顶的虚拟地址

        self.stack = Stack::new(
            Page::containing_address(virtual_stack_top_addr),
            STACK_DEF_PAGE,
        );
        // 调用stack.rs中的方法new，传递包含虚拟栈顶的页，并规定页数为默认用户栈页数

        let page_table = &mut self.page_table.mapper(); // 获取能够传递给elf::map_range 函数的page_table参数
        let frame_alloc = &mut *get_frame_alloc_for_sure(); // 获取能够传递给elf::map_range 函数的frame_alloc参数
        elf::map_range(
            stack_bot_addr,
            STACK_DEF_PAGE,
            page_table,
            frame_alloc,
            true,
        )
        .unwrap();
        // 这里一定要注意！因为map_range中，传入的addr是较小的那个，所以因为用户栈是向下增长，
        // 即栈顶地址大于栈底，所以这里应该传入stack_bot_addr
        // 第二项参数为用户栈默认页数，实际为1

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

    // 0x05 add:
    pub fn stack_start(&self) -> VirtAddr {
        self.stack.range.start.start_address()
    }

    pub fn fork(&self, stack_offset_count: u64) -> Self {
        // clone the page table context (see instructions)
        let owned_page_table = self.page_table.fork(); // 逐层调用

        let mapper = &mut owned_page_table.mapper();

        let alloc = &mut *get_frame_alloc_for_sure();

        Self {
            page_table: owned_page_table,
            stack: self.stack.fork(mapper, alloc, stack_offset_count),
            heap: self.heap.fork(),

            // do not share code info
            code: Vec::new(),
            code_usage: 0,
        }
    }

    // 0x07 add
    pub fn brk(&self, addr: Option<VirtAddr>) -> Option<VirtAddr> {
        self.heap.brk(
            addr,
            &mut self.page_table.mapper(),
            &mut get_frame_alloc_for_sure(),
        )
    }

    pub fn load_elf(&mut self, elf: &ElfFile) {
        let mapper = &mut self.page_table.mapper();

        let alloc = &mut *get_frame_alloc_for_sure();

        self.load_elf_code(elf, mapper, alloc); // 调用load_elf_code加载ELF文件的代码段
        self.stack.init(mapper, alloc);
    }

    fn load_elf_code(&mut self, elf: &ElfFile, mapper: MapperRef, alloc: FrameAllocatorRef) {
        // FIXME: make the `load_elf` function return the code pages
        self.code =
            elf::load_elf(elf, *PHYSICAL_OFFSET.get().unwrap(), mapper, alloc, true).unwrap();

        // FIXME: calculate code usage
        // code为一个Vec<PageRangeInclusive>，
        // .iter获取每一个对象，
        // 采用map(|page| page.count())计算单个对象页数
        // .sum()计算总页数
        let usage: usize = self.code.iter().map(|page| page.count()).sum();
        self.code_usage = usage as u64 * crate::memory::PAGE_SIZE;
    }

    pub(super) fn clean_up(&mut self) -> Result<(), UnmapError> {
        let mapper = &mut self.page_table.mapper();
        let dealloc = &mut *get_frame_alloc_for_sure();

        // statistics for logging and debugging
        // NOTE: you may need to implement `frames_recycled` by yourself
        let start_count = dealloc.frames_recycled();

        // TODO...
        self.stack.clean_up(mapper, dealloc)?; // 释放栈区

        if self.page_table.using_count() == 1 {
            // free heap
            self.heap.clean_up(mapper, dealloc)?;

            // free code
            for page_range in self.code.iter() {
                elf::unmap_range(*page_range, mapper, dealloc, true)?;
            }

            unsafe {
                // free P1-P3
                mapper.clean_up(dealloc);

                // free P4
                dealloc.deallocate_frame(self.page_table.reg.addr);
            }
        }

        // statistics for logging and debugging
        let end_count = dealloc.frames_recycled(); // 统计内存回收情况，打印调试信息

        debug!(
            "Recycled {}({:.3} MiB) frames, {}({:.3} MiB) frames in total.",
            end_count - start_count,
            ((end_count - start_count) * 4) as f32 / 1024.0,
            end_count,
            (end_count * 4) as f32 / 1024.0
        );

        Ok(())
    }
}

impl core::fmt::Debug for ProcessVm {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let (size, unit) = humanized_size(self.memory_usage());

        f.debug_struct("ProcessVm")
            .field("stack", &self.stack)
            .field("heap", &self.heap)
            .field("memory_usage", &format!("{} {}", size, unit))
            .field("page_table", &self.page_table)
            .finish()
    }
}

// 0x07 add: drop
impl Drop for ProcessVm {
    fn drop(&mut self) {
        if let Err(err) = self.clean_up() {
            error!("Failed to clean up process memory: {:?}", err);
        }
    }
}
