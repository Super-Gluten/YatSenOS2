use super::LocalApic;
use crate::interrupt::consts::*;
use crate::memory::address::physical_to_virtual;
use bit_field::BitField;
use core::fmt::{Debug, Error, Formatter};
use core::ptr::{read_volatile, write_volatile};
use x86::cpuid::CpuId;

/// Default physical address of xAPIC
pub const LAPIC_ADDR: u64 = 0xFEE00000;

pub struct XApic {
    addr: u64,
}

impl XApic {
    pub unsafe fn new(addr: u64) -> Self {
        // ?为什么不需要使用address.rs中的physical_to_virtual进行地址映射？
        XApic { addr }
    }

    unsafe fn read(&self, reg: u32) -> u32 {
        // 读取 APIC寄存器
        unsafe { read_volatile((self.addr + reg as u64) as *const u32) }
    }

    unsafe fn write(&mut self, reg: u32, value: u32) {
        unsafe {
            write_volatile((self.addr + reg as u64) as *mut u32, value);
            self.read(0x20);
        }
        // APIC要求：写入后通过读取Squrious Interrupt Vector Register
        // 来确保写入完成
    }
}

impl LocalApic for XApic {
    /// If this type APIC is supported
    fn support() -> bool {
        // FIXME: Check CPUID to see if xAPIC is supported.
        CpuId::new()
            .get_feature_info()
            .map(|f| f.has_apic())
            .unwrap_or(false)
    }

    /// Initialize the xAPIC for the current CPU.
    fn cpu_init(&mut self) {
        unsafe {
            // FIXME: Enable local APIC; set spurious interrupt vector.
            let mut spiv = self.read(0xF0);
            spiv |= 1 << 8; // set EN bit
            // clear and set Vector
            spiv &= !(0xFF);
            spiv |= Interrupts::IrqBase as u32 + Irq::Spurious as u32;
            self.write(0xF0, spiv);

            // FIXME: The timer repeatedly counts down at bus frequency
            // 设置计时器的相关寄存器
            self.write(0x3E0, 0b1011); // set Timer Divide to 1
            self.write(0x380, 0x20000); // set initial count to 0x20000
            let mut lvt_timer = self.read(0x320); // lvt_timer 位于0x320
            // clear and set Vector
            lvt_timer &= !(0xFF);

            // 计算新的中断向量号并设置
            lvt_timer |= Interrupts::IrqBase as u32 + Irq::Timer as u32;

            lvt_timer &= !(1 << 16); // clear Mask
            lvt_timer |= 1 << 17; // set Timer Periodic Mode
            self.write(0x320, lvt_timer);

            // FIXME: Disable logical interrupt lines (LINT0, LINT1)
            self.write(0x350, 1 << 16); // set Mask 禁用LVT LINT0
            self.write(0x360, 1 << 16); // set Mask 禁用LVT LINT1

            // FIXME: Disable performance counter overflow interrupts (PCINT)
            self.write(0x340, 1 << 16); // set mask 禁用LVT PCINT

            // FIXME: Map error interrupt to IRQ_ERROR.
            // lvt_error 位于0x370
            // 设置错误中断 LVT Error 到对应的中断向量号
            let mut lvt_error = self.read(0x370);
            // clear and set Vector
            lvt_error &= !(0xFF);
            lvt_error |= Interrupts::IrqBase as u32 + Irq::Timer as u32;
            self.write(0x370, lvt_error);

            // FIXME: Clear error status register (requires back-to-back writes).
            // 连续写入两次 0 以清除错误状态寄存器
            self.write(0x280, 0);
            self.write(0x280, 0);

            // FIXME: Ack any outstanding interrupts.
            // EOI寄存器位于0x0B0
            // 向 EOI 寄存器写入 0 以确认任何挂起的中断
            self.write(0x0B0, 0);

            // FIXME: Send an Init Level De-Assert to synchronise arbitration ID's.
            // 设置 ICR 寄存器
            self.write(0x310, 0); // set ICR 0x310
            const BCAST: u32 = 1 << 19;
            const INIT: u32 = 5 << 8;
            const TMLV: u32 = 1 << 15; // TM = 1, LV = 0
            self.write(0x300, BCAST | INIT | TMLV); // set ICR 0x300
            const DS: u32 = 1 << 12;
            while self.read(0x300) & DS != 0 {} // wait for delivery status

            // self.write(0x300, 1 << 19);
            // self.write(0x300, 0 << 18); // Destination Shorthand(bit 18-19): to 2
            // self.write(0x380, 1 << 10);
            // self.write(0x300, 0 << 9);
            // self.write(0x300, 0 << 8); // Delivery Mode(bit 8-10): to 5
            // self.write(0x300, 0 << 14); // Level(bit 14): to 0
            // self.write(0x300, 1 << 15); // Trigger Mode(bit 15): to 1
            // self.write(0x300, 0 << 12); // Delivery Status(bit 12): to 0

            // FIXME: Enable interrupts on the APIC (but not on the processor).
            self.write(0x080, 0); // 设置 TPR 寄存器为 0，允许接收中断。
        }

        // NOTE: Try to use bitflags! macro to set the flags.
    }

    fn id(&self) -> u32 {
        // NOTE: Maybe you can handle regs like `0x0300` as a const.
        unsafe { self.read(0x0020) >> 24 }
    }

    fn version(&self) -> u32 {
        unsafe { self.read(0x0030) }
    }

    fn icr(&self) -> u64 {
        unsafe { (self.read(0x0310) as u64) << 32 | self.read(0x0300) as u64 }
    }

    fn set_icr(&mut self, value: u64) {
        unsafe {
            while self.read(0x0300).get_bit(12) {}
            self.write(0x0310, (value >> 32) as u32);
            self.write(0x0300, value as u32);
            while self.read(0x0300).get_bit(12) {}
        }
    }

    fn eoi(&mut self) {
        unsafe {
            self.write(0x00B0, 0);
        }
    }
}

impl Debug for XApic {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        f.debug_struct("Xapic")
            .field("id", &self.id())
            .field("version", &self.version())
            .field("icr", &self.icr())
            .finish()
    }
}
