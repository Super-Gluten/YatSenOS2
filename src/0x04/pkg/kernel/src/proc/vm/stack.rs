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

use x86_64::{
    VirtAddr,
    structures::paging::{Page, mapper::MapToError, page::*},
};

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

        self.range = elf::map_range(STACK_INIT_BOT, STACK_DEF_PAGE, mapper, alloc).unwrap();
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
        let new_page = elf::map_range(
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
