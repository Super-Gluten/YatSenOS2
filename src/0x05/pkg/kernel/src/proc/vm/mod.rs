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
