mod apic;
mod syscall;
pub mod clock;
mod consts;
mod exceptions;
mod serial;

use crate::memory::physical_to_virtual;
use apic::*;
use x86_64::structures::idt::InterruptDescriptorTable;
use syscall::*;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            // CPU中断注册，时钟中断注册，串口输出中断注册
            exceptions::register_idt(&mut idt);
            // TODO: clock::register_idt(&mut idt);
            // TODO: serial::register_idt(&mut idt);
            clock::register_idt(&mut idt);
            serial::register_idt(&mut idt);
            syscall::register_idt(&mut idt); // 0x04 add
        }
        idt
    };
}

/// init interrupts system
pub fn init() {
    IDT.load();

    // FIXME: check and init APIC
    if XApic::support() {
        // support()用于检测xAPIC是否能被支持
        info!("XApic is supported");
        let cpuid = unsafe {
            // 本地apic地址：LAPIC_ADDR定义在apic/xapic.rs中
            let mut apic = XApic::new(LAPIC_ADDR);
            apic.cpu_init();
            apic.id() as u8 // 返回当前CPU的APIC ID
        };
        // FIXME: enable serial irq with IO APIC (use enable_irq)
        // 使用const.rs中Irq定义的第一个串口
        enable_irq(consts::Irq::Serial0 as u8, cpuid);
    } else {
        info!("XApic isn't supported!");
    }

    info!("Interrupts Initialized.");
}

#[inline(always)]
pub fn enable_irq(irq: u8, cpuid: u8) {
    let mut ioapic = unsafe { IoApic::new(physical_to_virtual(IOAPIC_ADDR)) };
    ioapic.enable(irq, cpuid);
}

#[inline(always)]
pub fn ack() {
    let mut lapic = unsafe { XApic::new(physical_to_virtual(LAPIC_ADDR)) };
    lapic.eoi();
}
