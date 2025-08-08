//! 物理内存管理模块
//!
//! 该模块提供操作系统底层的物理内存管理能力，包括：
//! - 物理内存的检测与统计
//! - 物理页帧的分配与回收
//! - 地址类型的转换与校验
//! - **全局描述符表 (GDT)** 和 **任务状态段 (TSS)** 的内存管理
//!
//! ## 主要组件
//! - BootInfoFrameAllocator：物理页帧分配器（基于引导信息初始化）
//! - [`GDT`]: 全局描述符表（包含代码/数据段选择子和 TSS）
//! - [`TSS`]: 任务状态段（管理特权栈和中断栈）

//! - 子模块：
//!   - `address`：地址类型转换与操作
//!   - `allocator`：内核堆分配器
//!   - `frames`：物理页帧管理（`FrameAllocator` trait 实现）
//!   - `gdt`：全局描述符表与任务状态段相关内存操作
//!
//! ## 关键接口
//! - 初始化: [`init`]
//! - 地址转换: [`physical_to_virtual`]
//!

pub mod address;
pub mod allocator;
mod frames;

pub mod gdt;

pub use address::*;
pub use frames::*;

use crate::humanized_size;

pub fn init(boot_info: &'static boot::BootInfo) {
    let memory_map = &boot_info.memory_map;

    let mut mem_size = 0;
    let mut usable_mem_size = 0;

    for item in memory_map.iter() {
        mem_size += item.page_count;
        if item.ty == boot::MemoryType::CONVENTIONAL {
            usable_mem_size += item.page_count;
        }
    }

    let (size, unit) = humanized_size(mem_size * PAGE_SIZE);
    info!("Physical Memory    : {:>7.*} {}", 3, size, unit);

    let (size, unit) = humanized_size(usable_mem_size * PAGE_SIZE);
    info!("Free Usable Memory : {:>7.*} {}", 3, size, unit);

    unsafe {
        init_FRAME_ALLOCATOR(BootInfoFrameAllocator::init(
            memory_map,
            usable_mem_size as usize,
        ));
    }

    info!("Frame Allocator initialized.");
}
