//! 中断处理模块
//!
//! 该模块提供了操作系统核心的中断处理功能，包括：
//! - 中断描述符表 (IDT) 的初始化与管理
//! - 局部 APIC (LAPIC) 和 I/O APIC 的配置
//! - 时钟、异常和串口等中断的注册与处理
//!
//! ## 主要组件
//! - [`IDT`]: 全局中断描述符表（通过 `lazy_static!` 初始化）
//! - [`XApic`]/[`IoApic`]: APIC 驱动实现（xAPIC 模式）
//! - 子模块:
//!   - `apic`: Advanced Programmable Interrupt Controller驱动实现
//!   - `clock`: 时钟中断
//!   - `consts`: 相关常量定义
//!   - `exceptions`: CPU 异常处理
//!   - `serial`: 串口中断
//!
//! ## 关键接口
//! - 初始化: [`init`]
//! - 中断控制: [`enable_irq`], [`ack`]
//!

mod apic;
pub mod clock;
mod consts;
mod exceptions;
mod serial;

use crate::memory::physical_to_virtual;
use apic::*;
use x86_64::structures::idt::InterruptDescriptorTable;

lazy_static! {
    /// Initialize the Interrupt Descriptor Table (IDT) and register for different interrupt
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            exceptions::register_idt(&mut idt);
            clock::register_idt(&mut idt);
            serial::register_idt(&mut idt);
        }
        idt
    };
}

/// init interrupts system
pub fn init() {
    IDT.load();

    // check and init APIC (use XApic function)
    if XApic::support() {
        info!("XApic is supported");
        let cpuid = unsafe {
            let mut apic = XApic::new(physical_to_virtual(LAPIC_ADDR));
            apic.cpu_init();
            apic.id() as u8
        }; // get current CPU's APIC ID

        // enable serial irq with IO APIC (use enable_irq)
        enable_irq(consts::Irq::Serial0 as u8, cpuid);
    } else {
        error!("XApic isn't supported!");
    }

    info!("Interrupts Initialized.");
}

/// Enable specific interrupt request and route it to the target CPU
#[inline(always)]
pub fn enable_irq(irq: u8, cpuid: u8) {
    let mut ioapic = unsafe { IoApic::new(physical_to_virtual(IOAPIC_ADDR)) };
    ioapic.enable(irq, cpuid);
}

/// Notify the LAPIC that the current interrupt has been processed suceesfully
#[inline(always)]
pub fn ack() {
    let mut lapic = unsafe { XApic::new(physical_to_virtual(LAPIC_ADDR)) };
    lapic.eoi();
}
