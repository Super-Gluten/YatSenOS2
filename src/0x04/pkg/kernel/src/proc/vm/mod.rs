use alloc::format;
use x86_64::{
    VirtAddr,
    structures::paging::{page::*, *},
};

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
        info!("{}", self.stack.memory_usage());
        self
    }

    // Initializes the process stack for the given ProcessId
    pub fn init_proc_stack(&mut self, pid: ProcessId) -> VirtAddr {
        // calculate the stack for pid
        //
        // 1. Calculate the physical address of stack
        let stack_top_addr = STACK_INIT_TOP - STACK_MAX_SIZE * (pid.0 as u64 - 1);
        let stack_bot_addr = STACK_INIT_BOT - STACK_MAX_SIZE * (pid.0 as u64 - 1);
        info!("top {:?} bot {:?}", stack_top_addr, stack_bot_addr);

        // 2. Virtualize the stack_top and create the stack
        let virtual_stack_top_addr = VirtAddr::new(stack_top_addr);

        self.stack = Stack::new(
            Page::containing_address(virtual_stack_top_addr),
            STACK_DEF_PAGE,
        );

        // 3. Use map_range to perform memory mapping on the stack
        //
        // # Attention:
        // The stack grow downwards in the kernel, so the map_range use stack_bot_addr
        // because it's smaller than stack_top_addr
        let page_table = &mut self.page_table.mapper();
        let frame_alloc = &mut *get_frame_alloc_for_sure();
        elf::map_range(stack_bot_addr, STACK_DEF_PAGE, page_table, frame_alloc).unwrap();

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
