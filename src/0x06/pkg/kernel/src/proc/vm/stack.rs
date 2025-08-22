use elf::user_map_range;
use x86_64::{
    VirtAddr,
    structures::paging::{Page, mapper::MapToError, page::*},
};
 use core::ptr::copy_nonoverlapping;

use super::{FrameAllocatorRef, MapperRef};

/// | Constant               | Value                        | Description                              |
/// |------------------------|------------------------------|------------------------------------------|
/// | **用户栈常量**          | bot..0x2000_0000_0000        | top..0x3fff_ffff_ffff                   |
/// | STACK_MAX              | 0x4000_0000_0000             | 用户栈最大的虚拟地址边界                 |
/// | STACK_MAX_PAGES        | 0x100000                     | 用户栈的最大页数                         |
/// | STACK_MAX_SIZE         | 0x1_0000_0000                | 由最大页数 × 页面大小得出                |
/// | STACK_START_MASK       | !(STACK_MAX_SIZE - 1)        | 用于对齐栈底地址的掩码                   |
/// | STACK_DEF_BOT          | 0x2000_0000_0000             | 用户栈栈底地址                           |
/// | STACK_DEF_PAGE         | 1                            | 默认用户栈分配栈的页数                   |
/// | STACK_DEF_SIZE         | 0x1000                       | 默认用户栈的大小                         |
/// | STACK_INIT_BOT         | STACK_MAX - STACK_DEF_SIZE   | 初始用户栈栈底                           |
/// | STACK_INIT_TOP         | STACK_MAX - 8                | 初始用户栈栈顶，-8 是为了页对齐          |
/// |-------                 |                              |                                          |
/// | **内核栈常量**          | bot..0xffff_ff01_0000_0000   | top..0xffff_ff01_ffff_ffff               |
/// | KSTACK_MAX             | 0xffff_ff02_0000_0000        | 内核栈最大的虚拟地址边界                 |
/// | KSTACK_DEF_BOT         | 0xffff_ff01_0000_0000        | 默认内核栈栈底                           |
/// | KSTACK_DEF_PAGE        | 512                          | 默认内核栈页数                           |
/// | KSTACK_DEF_SIZE        | 0x0020_0000 = 2MiB           | 默认内核栈的大小                         |
/// | KSTACK_INIT_BOT        | KSTACK_MAX - KSTACK_DEF_SIZE | 初始内核栈栈底                           |
/// | KSTACK_INIT_TOP        | KSTACK_MAX - 8               | 初始内核栈栈顶，-8 是为了页对齐          |

// | ------                 |                                                           |                            |
// | **页面相关**            |                                                          |                            |
// | STACK_INIT_TOP_PAGE    | Page::containing_address(VirtAddr::new(STACK_INIT_TOP))  | 用户栈初始栈顶所在页面       |
// | KSTACK_INIT_PAGE       | Page::containing_address(VirtAddr::new(KSTACK_INIT_BOT)) | 内核栈初始栈底所在页面       |
// | KSTACK_INIT_TOP_PAGE   | Page::containing_address(VirtAddr::new(KSTACK_INIT_TOP)) | 内核栈初始栈顶所在页面       |

// 0xffff_ff00_0000_0000 is the kernel's address space
pub const STACK_MAX: u64 = 0x4000_0000_0000;
pub const STACK_MAX_PAGES: u64 = 0x100000;
pub const STACK_MAX_SIZE: u64 = STACK_MAX_PAGES * crate::memory::PAGE_SIZE;
pub const STACK_START_MASK: u64 = !(STACK_MAX_SIZE - 1);

// [bot..0x2000_0000_0000..top..0x3fff_ffff_ffff]
// init stack
pub const STACK_DEF_BOT: u64 = STACK_MAX - STACK_MAX_SIZE;
pub const STACK_DEF_PAGE: u64 = 1;
pub const STACK_DEF_SIZE: u64 = STACK_DEF_PAGE * crate::memory::PAGE_SIZE;

pub const STACK_INIT_BOT: u64 = STACK_MAX - STACK_DEF_SIZE;
pub const STACK_INIT_TOP: u64 = STACK_MAX - 8;
const STACK_INIT_TOP_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(STACK_INIT_TOP));

// [bot..0xffffff0100000000..top..0xffffff01ffffffff]
// kernel stack

pub const KSTACK_MAX: u64 = 0xffff_ff02_0000_0000;
pub const KSTACK_DEF_BOT: u64 = KSTACK_MAX - STACK_MAX_SIZE;

pub const KSTACK_DEF_PAGE: u64 = 512;
pub const KSTACK_DEF_SIZE: u64 = KSTACK_DEF_PAGE * crate::memory::PAGE_SIZE;

pub const KSTACK_INIT_BOT: u64 = KSTACK_MAX - KSTACK_DEF_SIZE;
pub const KSTACK_INIT_TOP: u64 = KSTACK_MAX - 8;

const KSTACK_INIT_PAGE: Page<Size4KiB> = Page::containing_address(VirtAddr::new(KSTACK_INIT_BOT));
const KSTACK_INIT_TOP_PAGE: Page<Size4KiB> =
    Page::containing_address(VirtAddr::new(KSTACK_INIT_TOP));

pub struct Stack {
    range: PageRange<Size4KiB>,
    usage: u64,
}

impl Stack {
    // Create a stack containing `size` page from the `top` page
    //
    // Satisfy the downward growth of the stack by `top-size+1` as first parameter
    // # Attention
    // range() doesn't include second parameter, which is the reason why we use `top+1`
    pub fn new(top: Page, size: u64) -> Self {
        Self {
            range: Page::range(top - size + 1, top + 1),
            usage: size,
        }
    }

    pub const fn empty() -> Self {
        Self {
            range: Page::range(STACK_INIT_TOP_PAGE, STACK_INIT_TOP_PAGE),
            usage: 0,
        }
    }

    // Build kernel stack using the kernel stack constant
    pub const fn kstack() -> Self {
        Self {
            range: Page::range(KSTACK_INIT_PAGE, KSTACK_INIT_TOP_PAGE),
            usage: KSTACK_DEF_PAGE,
        }
    }

    // Initialize the stack
    pub fn init(&mut self, mapper: MapperRef, alloc: FrameAllocatorRef) {
        debug_assert!(self.usage == 0, "Stack is not empty.");

        self.range = user_map_range(STACK_INIT_BOT, STACK_DEF_PAGE, mapper, alloc).unwrap();
        self.usage = STACK_DEF_PAGE;
    }

    /// Determined whether page fault has been successfully resolved
    ///
    /// # Returns
    /// - `false` if:
    ///  - the address is out of the stack space of current process
    ///  - failed to grow stack
    pub fn handle_page_fault(
        &mut self,
        addr: VirtAddr,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> bool {
        if !self.is_on_stack(addr) {
            return false;
        }

        if let Err(m) = self.grow_stack(addr, mapper, alloc) {
            error!("Grow stack failed: {:?}", m);
            return false;
        }
        true
    }

    /// Determine whether page fault occurred in the stack of current process
    fn is_on_stack(&self, addr: VirtAddr) -> bool {
        let addr = addr.as_u64();
        let cur_stack_bot = self.range.start.start_address().as_u64();
        trace!("Current stack bot: {:#x}", cur_stack_bot);
        trace!("Address to access: {:#x}", addr);
        addr & STACK_START_MASK == cur_stack_bot & STACK_START_MASK
    }

    fn grow_stack(
        &mut self,
        addr: VirtAddr,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
    ) -> Result<(), MapToError<Size4KiB>> {
        // When in debug-mode, use fn is_on_stack() to enhance robustness
        debug_assert!(self.is_on_stack(addr), "Address is not on stack.");

        // grow stack for page fault
        //
        // 1. Calculate on which page the fault occurred
        let aim_page = Page::<Size4KiB>::containing_address(addr);

        // 2. Calculate the number of page that need to be increased
        let count_alloc = (self.range.start - aim_page)
            .try_into()
            .expect("Failed to convert u64 to usize");

        // 3. Use map_range to perform memory mapping on new increased page
        //
        // # Attention
        // The os is page aligned, using addr.as_u64() as the first parameter for map_range()
        // will disrupt page alignment and memory layout
        let new_page = user_map_range(
            aim_page.start_address().as_u64(),
            count_alloc,
            mapper,
            alloc,
        )?;

        // 4. Add the usage of page and Merge pages
        self.usage += count_alloc;
        self.range = Page::range(new_page.start, self.range.end);

        // only when the stack usage reach several hundred, the message will be info
        if self.usage % 100 == 0 {
            info!(
                "Grow Stack: new start {:?}, end {:?}, usage {:?} pages",
                self.range.start, self.range.end, self.usage
            );
        }

        Ok(())
    }

    pub fn memory_usage(&self) -> u64 {
        self.usage * crate::memory::PAGE_SIZE
    }

    pub fn stack_start(&self) -> VirtAddr {
        self.range.start.start_address()
    }

    pub fn stack_usage(&self) -> u64 {
        self.usage
    }

    /// Allocate free stack space for child processes
    /// 
    /// - `stack_offset_count`: page offset related to the number of child processes
    pub fn fork(
        &self,
        mapper: MapperRef,
        alloc: FrameAllocatorRef,
        stack_offset_count: u64,
    ) -> Self {
        // 1. alloc & map new stack for child (see instructions)
        let mut child_stack_top = (self.range.start - stack_offset_count).start_address();
        let child_usage = self.usage;
        // while the stack space isn't free for process,
        // the `child_stack_top` grows downwards
        while user_map_range(
            child_stack_top.as_u64(), 
            child_usage, 
            mapper, 
            alloc,
        ).is_err() {
            child_stack_top -= STACK_MAX_SIZE;
            trace!("Mapping is not empty, stack grows down to {:#x}", child_stack_top.as_u64());
        }

        // 2. copy the *entire stack* from parent to child
        self.clone_range(
            self.range.start.start_address().as_u64(),
            child_stack_top.as_u64(), 
            child_usage,
        );

        // 3. return the new stack
        let child_start_page = Page::containing_address(child_stack_top);
        let child_end_page = child_start_page + child_usage;
        let child_range = Page::range(child_start_page, child_end_page);

        Self {
            range: child_range,
            usage: child_usage,
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
