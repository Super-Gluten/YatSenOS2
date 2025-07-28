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
            let mut apic = XApic::new(LAPIC_ADDR);
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
