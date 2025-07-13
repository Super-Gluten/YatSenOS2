use alloc::format;
use x86_64::{
    VirtAddr,
    structures::paging::{page::*, *},
};

use crate::{humanized_size, memory::*};

pub mod stack;

use self::stack::*;

use super::{PageTableContext, ProcessId};

use xmas_elf::ElfFile;

type MapperRef<'a> = &'a mut OffsetPageTable<'static>;
type FrameAllocatorRef<'a> = &'a mut BootInfoFrameAllocator;

pub struct ProcessVm {
    // page table is shared by parent and child
    pub(super) page_table: PageTableContext, // 使用paging.rs中的结构体

    // stack is pre-process allocated
    pub(super) stack: Stack, // 使用stack.rs中的结构体
}

impl ProcessVm {
    pub fn new(page_table: PageTableContext) -> Self {
        Self {
            page_table,
            stack: Stack::empty(),
        }
    }

    pub fn init_kernel_vm(mut self) -> Self {
        // TODO: record kernel code usage
        self.stack = Stack::kstack();
        self
    }

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
    pub fn load_elf(&mut self, elf: &ElfFile) {
        let mapper = &mut self.page_table.mapper();
        let alloc = &mut *get_frame_alloc_for_sure();

        // FIXME: load elf to process pagetable
        elf::load_elf(
            elf,
            *PHYSICAL_OFFSET.get().unwrap(), // 克隆内核的地址偏移量
            mapper,
            alloc,
            true, // 因为调用本函数的都是用户进程，所以user_access都是true
        )
        .unwrap();

        self.stack.init(mapper, alloc);
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
