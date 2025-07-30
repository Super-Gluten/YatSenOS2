use super::LocalApic;
use crate::interrupt::consts::*;
use crate::memory::address::physical_to_virtual;
use bit_field::BitField;
use bitflags::bitflags;
use core::fmt::{Debug, Error, Formatter};
use core::ptr::{read_volatile, write_volatile};
use x86::cpuid::CpuId;

/// Default physical address of xAPIC
pub const LAPIC_ADDR: u64 = 0xFEE00000;

/// Mask used to clear interrupt vector numbers
const IRQ_CLEAR: u32 = !(0xFF);

pub struct XApic {
    addr: u64,
}

impl XApic {
    /// Registers in Apic
    pub const ID: u32 = 0x020;
    pub const VERSION: u32 = 0x030;
    pub const TPR: u32 = 0x080;
    pub const EOI: u32 = 0x0B0;
    pub const SPIV: u32 = 0x0F0;
    pub const ERROR_STATUS: u32 = 0x280;
    pub const ICR_LOW: u32 = 0x300;
    pub const ICR_HIGH: u32 = 0x310;
    pub const LVT_TIMER: u32 = 0x320;
    pub const LVT_PCINT: u32 = 0x340;
    pub const LVT_LINT0: u32 = 0x350;
    pub const LVT_LINT1: u32 = 0x360;
    pub const LVT_ERROR: u32 = 0x370;
    pub const TICR: u32 = 0x380;
    pub const TDCR: u32 = 0x3E0;

    pub unsafe fn new(addr: u64) -> Self {
        XApic { addr }
    }

    unsafe fn read(&self, reg: u32) -> u32 {
        unsafe { read_volatile((self.addr + reg as u64) as *const u32) }
    }

    unsafe fn write(&mut self, reg: u32, value: u32) {
        unsafe {
            write_volatile((self.addr + reg as u64) as *mut u32, value);
            self.read(0x20);
        }
    }
}

bitflags! {
    pub struct SpivFlags: u32 {
        const ENABLE = 1 << 8;
        const FOUCS_DISABLE = 1 << 7;
    }
}

bitflags! {
    pub struct LocalVectorTableFlags: u32 {
        const TIME_DIVIDE1  = 0b1011;
        const TIME_DIVIDE2  = 0b0000;
        const TIME_DIVIDE4  = 0b0001;
        const TIME_DIVIDE8  = 0b0010;
        const TIME_DIVIDE16 = 0b0011;
        const TIME_DIVIDE32 = 0b1000;
        const TIME_DIVIDE64 = 0b1001;
        const TIME_DIVIDE128 = 0b1010;

        const DS = 1 << 12;
        const MASK = 1 << 16;
        const TP = 1 << 17;
    }
}

bitflags! {
    pub struct InterruptCommandFlags: u32 {
        const DM = 5 << 8;
        const DS = 1 << 12;
        const LEVEL0 = 0 << 14;
        const LEVEL1 = 1 << 14;
        const TM = 1 << 15;
        const DSH = 1 << 19;
    }
}

impl LocalApic for XApic {
    /// If this type APIC is supported
    fn support() -> bool {
        // Check CPUID to see if xAPIC is supported.
        CpuId::new()
            .get_feature_info()
            .map(|f| f.has_apic())
            .unwrap_or(false)
    }

    /// Initialize the xAPIC for the current CPU.
    fn cpu_init(&mut self) {
        unsafe {
            // Enable local APIC; set spurious interrupt vector.
            let mut spiv = self.read(Self::SPIV);
            spiv |= SpivFlags::ENABLE.bits(); // set EN bit

            // clear and set Vector
            spiv &= IRQ_CLEAR;
            spiv |= SPURIOUS_INTERRUPT_VECTOR;
            self.write(Self::SPIV, spiv);

            // The timer repeatedly counts down at bus frequency
            self.write(Self::TDCR, LocalVectorTableFlags::TIME_DIVIDE1.bits()); // set Timer Divide to 1
            self.write(Self::TICR, 0x20000); // set initial count to 0x20000
            let mut lvt_timer = self.read(Self::LVT_TIMER); // lvt_timer located in 0x320

            // clear and set Vector
            lvt_timer &= IRQ_CLEAR;
            lvt_timer |= CLOCK_INTERRUPT_VECTOR as u32;

            lvt_timer &= !LocalVectorTableFlags::MASK.bits(); // clear Mask
            lvt_timer |= LocalVectorTableFlags::TP.bits(); // set Timer Periodic Mode
            self.write(Self::LVT_TIMER, lvt_timer);

            // Disable logical interrupt lines (LINT0, LINT1)
            self.write(Self::LVT_LINT0, LocalVectorTableFlags::MASK.bits()); // set Mask to disable LVT LINT0
            self.write(Self::LVT_LINT1, LocalVectorTableFlags::MASK.bits()); // set Mask to disable LVT LINT1

            // Disable performance counter overflow interrupts (PCINT)
            self.write(Self::LVT_PCINT, LocalVectorTableFlags::MASK.bits()); // set Mask to disable LVT PCINT

            // Map error interrupt to IRQ_ERROR.
            let mut lvt_error = self.read(Self::LVT_ERROR); // set LVT_Error 0x370

            // clear and set Vector
            lvt_error &= IRQ_CLEAR;
            lvt_error |= CLOCK_INTERRUPT_VECTOR as u32;
            self.write(Self::LVT_ERROR, lvt_error);

            // Clear error status register (requires back-to-back writes).
            self.write(Self::ERROR_STATUS, 0);
            self.write(Self::ERROR_STATUS, 0);

            // Ack any outstanding interrupts.
            self.eoi();

            // Send an Init Level De-Assert to synchronise arbitration ID's.
            // Send an Init Level De-Assert to synchronise arbitration ID's.
            self.write(Self::ICR_HIGH, 0); // set ICR_HIGH as 0
            let mut icr_low: u32 = InterruptCommandFlags::DM.bits() |
                // InterruptCommandFlags::LEVEL0.bits() |
                InterruptCommandFlags::TM.bits() |
                InterruptCommandFlags::DSH.bits();
            self.write(Self::ICR_LOW, icr_low);
            while self.read(Self::ICR_LOW) & InterruptCommandFlags::DS.bits() != 0 {} // wait for delivery status

            // Enable interrupts on the APIC (but not on the processor).
            self.write(Self::TPR, 0); // set TPR as 0
        }
    }

    fn id(&self) -> u32 {
        unsafe { self.read(Self::ID) >> 24 }
    }

    fn version(&self) -> u32 {
        unsafe { self.read(Self::VERSION) }
    }

    fn icr(&self) -> u64 {
        unsafe { (self.read(Self::ICR_HIGH) as u64) << 32 | self.read(Self::ICR_LOW) as u64 }
    }

    fn set_icr(&mut self, value: u64) {
        unsafe {
            while self.read(Self::ICR_LOW).get_bit(12) {}
            self.write(Self::ICR_HIGH, (value >> 32) as u32);
            self.write(Self::ICR_LOW, value as u32);
            while self.read(Self::ICR_LOW).get_bit(12) {}
        }
    }

    fn eoi(&mut self) {
        unsafe {
            self.write(Self::EOI, 0);
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
