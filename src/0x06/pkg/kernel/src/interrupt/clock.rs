use super::consts::*;
use crate::memory::gdt;
use crate::proc::ProcessContext;
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

/// Register clock interrupt handler
pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    unsafe {
        idt[CLOCK_INTERRUPT_VECTOR]
            .set_handler_fn(clock_handler)
            .set_stack_index(gdt::CLOCK_IST_INDEX);
    }
}

// Use as_handler macro to redefine the clock interrupt handler
pub extern "C" fn clock(mut context: ProcessContext) {
    crate::proc::switch(&mut context);
    super::ack();
}
as_handler!(clock);

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
