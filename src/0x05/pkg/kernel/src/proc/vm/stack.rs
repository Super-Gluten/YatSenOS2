use x86_64::{
    structures::paging::{mapper::MapToError, page::*, Page},
    VirtAddr,
};

use super::{FrameAllocatorRef, MapperRef};

use core::ptr::copy_nonoverlapping; // 0x05 add：用于clone_range

// 0xffff_ff00_0000_0000 is the kernel's address space
// crate::memory::PAGE_SIZE = 4096 = 0x1000; 定义在address.rs中
pub const STACK_MAX: u64 = 0x4000_0000_0000; // 用户栈最大的虚拟地址边界
pub const STACK_MAX_PAGES: u64 = 0x100000; // 用户栈的最大页数
pub const STACK_MAX_SIZE: u64 = STACK_MAX_PAGES * crate::memory::PAGE_SIZE; // 用户栈的最大大小
// STACK_MAX_SIZE = 0X1_0000_0000 ，由最大页数决定
pub const STACK_START_MASK: u64 = !(STACK_MAX_SIZE - 1); // 用于对齐栈底地址的掩码
// 用于将地址向下对齐到4GB边界

// [bot..0x2000_0000_0000..top..0x3fff_ffff_ffff]
// init stack 
// 请注意用户栈向下增长
pub const STACK_DEF_BOT: u64 = STACK_MAX - STACK_MAX_SIZE; // 用户栈栈底地址
pub const STACK_DEF_PAGE: u64 = 1; // 默认用户栈分配栈的页数
pub const STACK_DEF_SIZE: u64 = STACK_DEF_PAGE * crate::memory::PAGE_SIZE; // 默认用户栈的大小

pub const STACK_INIT_BOT: u64 = STACK_MAX - STACK_DEF_SIZE; // 初始用户栈栈底
pub const STACK_INIT_TOP: u64 = STACK_MAX - 8; // 初始用户栈栈顶
// why -8  => 满足64位系统的对齐要求
const STACK_INIT_TOP_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(STACK_INIT_TOP));
// 包含STACK_INIT_TOP地址的4KB页，用于页表映射?

// [bot..0xffffff0100000000..top..0xffffff01ffffffff]
// kernel stack
// 根据上述的定义用户栈的方式，推导内核栈的定义方式即可
pub const KSTACK_MAX: u64 = 0xffff_ff02_0000_0000; // 内核栈最大的虚拟地址边界
pub const KSTACK_DEF_BOT: u64 = KSTACK_MAX - STACK_MAX_SIZE; // 默认内核栈栈底
// KSTACK_DEF_BOT = 0xffff_ff01_0000_0000
pub const KSTACK_DEF_PAGE: u64 = 512 /* FIXME: decide on the boot config*/;
// 由于在boot/config.rs中设置的 kernel_stack_size = 512，实际映射为2MB
pub const KSTACK_DEF_SIZE: u64 = KSTACK_DEF_PAGE * crate::memory::PAGE_SIZE; // 默认内核栈的大小

pub const KSTACK_INIT_BOT: u64 = KSTACK_MAX - KSTACK_DEF_SIZE; // 初始内核栈栈底
pub const KSTACK_INIT_TOP: u64 = KSTACK_MAX - 8; // 初始内核栈栈顶 -8原因同上

const KSTACK_INIT_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(KSTACK_INIT_BOT));
const KSTACK_INIT_TOP_PAGE: Page<Size4KiB> =
    Page::containing_address(VirtAddr::new(KSTACK_INIT_TOP));

pub struct Stack {
    pub(super) range: PageRange<Size4KiB>, // 0x05要求设置为pub(super)供cm/mod.rs使用
    // PageRange<Size4KiB>是x86_64中定义的 表示连续页面范围的结构体
    // 将物理/虚拟内存地址范围表示为页面集合，便于计算和内存集成管理
    usage: u64, // 使用的页数
}

impl Stack {
    pub fn new(top: Page, size: u64) -> Self {
        Self {
            range: Page::range(top - size + 1, top + 1),
            // Page是x86_64中定义的 表示虚拟内存页的核心类型
            // 这里使用的range()函数是 页范围迭代
            // 意味着这里为Stack的range成员创建了一个从 a到b的连续页范围PageRange对象
            usage: size, // 栈内包含的页数
        } // 函数功能：从top页号开始，向下创建页数为size的栈
    }

    pub const fn empty() -> Self {
        Self {
            range: Page::range(STACK_INIT_TOP_PAGE, STACK_INIT_TOP_PAGE),
            usage: 0,
        }
    } // 创建一个初始为空，即页数为0的用户栈

    pub const fn kstack() -> Self {
        Self {
            range: Page::range(KSTACK_INIT_PAGE, KSTACK_INIT_TOP_PAGE),
            usage: KSTACK_DEF_PAGE,
        }
    } // 创建内核栈，这里使用的都是上述定义的内核栈相关常量

    pub fn init(&mut self, mapper: MapperRef, alloc: FrameAllocatorRef) {
        debug_assert!(self.usage == 0, "Stack is not empty.");

        self.range = elf::map_range(STACK_INIT_BOT, STACK_DEF_PAGE, mapper, alloc, true).unwrap();
        // 调用函数为elf/src/lib.rs中的map_range why
        self.usage = STACK_DEF_PAGE; // 默认用户栈的页数
    }

    pub fn handle_page_fault(
        &mut self,
        addr: VirtAddr,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> bool {
        if !self.is_on_stack(addr) {
            return false;
        } // 判断缺页异常的地址是否在当前进程的栈空间中，不在则直接返回false

        if let Err(m) = self.grow_stack(addr, mapper, alloc) {
            error!("Grow stack failed: {:?}", m);
            return false;
        } // 如果堆栈失败，则返回false
        true
    }

    fn is_on_stack(&self, addr: VirtAddr) -> bool {
        let addr = addr.as_u64();
        let cur_stack_bot = self.range.start.start_address().as_u64();
        trace!("Current stack bot: {:#x}", cur_stack_bot);
        trace!("Address to access: {:#x}", addr);
        addr & STACK_START_MASK == cur_stack_bot & STACK_START_MASK
    } // 判断当前地址是否在当前进程的栈空间中

    fn grow_stack(
        &mut self,
        addr: VirtAddr,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> Result<(), MapToError<Size4KiB>> {
        debug_assert!(self.is_on_stack(addr), "Address is not on stack.");
        // 调试模式下，判断发生缺页错误的地址是否在当前进程的栈空间下

        // FIXME: grow stack for page fault
        let aim_page = Page::<Size4KiB>::containing_address(addr); // 计算异常地址所在页面
        let count_alloc = (self.range.start - aim_page)
            .try_into()
            .expect("Failed to convert u64 to usize"); // 计算需要增长的页面数量
        // let new_page = elf::map_range(addr.as_u64(), count_alloc, mapper, alloc)?; 
        // 这里不能采用addr.as_u64()，而应该采用包含addr的页面的起始地址作为正确的u64传入
        let new_page = elf::map_range(
            aim_page.start_address().as_u64(), 
            count_alloc, 
            mapper, 
            alloc,
            true,
        )?;
        // 调用elf/lib.rs中的map_range函数求得相应Page

        self.usage += count_alloc; // 栈的页数使用量增加
        self.range = Page::range(new_page.start, self.range.end); // 页的合并

        if self.usage % 0x1000 == 0 || self.usage % 0x1 == 0{
            info!(
                "Grow Stack: new start {:?}, end {:?}, usage {:?} pages", 
                self.range.start, self.range.end, self.usage);
        }
        Ok(())
    }

    pub fn memory_usage(&self) -> u64 {
        self.usage * crate::memory::PAGE_SIZE
    } // 计算栈的内存大小 = 页数 * 页的大小

    // 0x05 add:
    pub fn fork(
        &self,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
        stack_offset_count: u64,
    ) -> Self {
        // FIXME: alloc & map new stack for child (see instructions)
        let mut child_stack_top = (self.range.start - stack_offset_count).start_address();
        let child_stack_usage = self.usage;
        // FIXME: copy the *entire stack* from parent to child
        while elf::map_range(
            child_stack_top.as_u64(),
            child_stack_usage,
            mapper,
            alloc,
            true
            ).is_err() {
                trace!("Map thread stack to {:#x} failed.", child_stack_top);
                child_stack_top -= STACK_MAX_SIZE; // 栈区向低位增长
            };
        // FIXME: return the new stack
        self.clone_range(
            self.range.start.start_address().as_u64(),
            child_stack_top.as_u64(),
            child_stack_usage
        );
        let child_range_start = Page::containing_address(child_stack_top);
        let child_range_end = child_range_start + child_stack_usage;
        let child_range = Page::range(child_range_start, child_range_end);
        Self {
            range: child_range,
            usage: child_stack_usage
        }
    }


    /// Clone a range of memory
    ///
    /// - `src_addr`: the address of the source memory
    /// - `dest_addr`: the address of the target memory
    /// - `size`: the count of pages to be cloned
    fn clone_range(&self, cur_addr: u64, dest_addr: u64, size: u64) {
        trace!("Clone range: {:#x} -> {:#x}", cur_addr, dest_addr);
        unsafe {
            copy_nonoverlapping::<u64>(
                cur_addr as *mut u64,
                dest_addr as *mut u64,
                (size * Size4KiB::SIZE / 8) as usize,
            );
        }
    }
}

impl core::fmt::Debug for Stack {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        f.debug_struct("Stack")
            .field(
                "top",
                &format_args!("{:#x}", self.range.end.start_address().as_u64()),
            )
            .field(
                "bot",
                &format_args!("{:#x}", self.range.start.start_address().as_u64()),
            )
            .finish()
    }
}
