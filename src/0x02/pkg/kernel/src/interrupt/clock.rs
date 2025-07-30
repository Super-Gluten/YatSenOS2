use super::consts::*;
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

/// Register clock interrupt handler
pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt[CLOCK_INTERRUPT_VECTOR].set_handler_fn(clock_handler);
}

pub extern "x86-interrupt" fn clock_handler(_sf: InterruptStackFrame) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // interrupt masking to prevent interrupt nesting
        if inc_counter() % 0x100000 == 0 {
            // if inc_counter() % 0x10000 == 0 {
            info!("Tick! @{}", read_counter());
        }
        super::ack();
    });
}

// Use a global atomic counter to ensure atomicity
static COUNTER: AtomicU64 = AtomicU64::new(0);

#[inline]
pub fn read_counter() -> u64 {
    // safely load counter value
    COUNTER.load(Ordering::SeqCst)
}

#[inline]
pub fn inc_counter() -> u64 {
    // read counter value and increase it
    COUNTER.fetch_add(1, Ordering::SeqCst)
}
