use crate::memory::*;
use core::arch::asm;
use x86_64::VirtAddr;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    // set_handler_fn是将特定中断、异常绑定到自定义处理函数的方法
    // set_stack_index是为高风险异常分配独立栈空间的方法
    idt.divide_error.set_handler_fn(divide_error_handler);
    idt.double_fault
        .set_handler_fn(double_fault_handler)
        .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
    idt.page_fault
        .set_handler_fn(page_fault_handler)
        .set_stack_index(gdt::PAGE_FAULT_IST_INDEX);

    // TODO: you should handle more exceptions here
    // especially general protection fault (GPF)
    // see: https://wiki.osdev.org/Exceptions

    idt.general_protection_fault
        .set_handler_fn(general_protection_fault_handler);
}

pub extern "x86-interrupt" fn divide_error_handler(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: DIVIDE ERROR\n\n{:#?}", stack_frame);
}

pub extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!(
        "EXCEPTION: DOUBLE FAULT, ERROR_CODE: 0x{:016x}\n\n{:#?}",
        error_code, stack_frame
    );
}

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    err_code: PageFaultErrorCode,
) {
    panic!(
        "EXCEPTION: PAGE FAULT, ERROR_CODE: {:?}\n\nTrying to access: {:#x}\n{:#?}",
        err_code,
        Cr2::read().unwrap_or(VirtAddr::new_truncate(0xdeadbeef)),
        stack_frame
    );
}

// pub extern "x86-interrupt" fn general_protection_fault_handler(
//     stack_frame: InterruptStackFrame,
//     err_code: u64,
//     // 在 idt.rs中expected为u64而非PageFaultErrorCode
// ) {
//     panic!(
//         "EXCEPTION: GENERAL PROTECTION FAULT, ERROR_CODE: 0x{:016x}\n\n{:#?}",
//         err_code, stack_frame
//     );
// }

pub extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    err_code: u64,
) {
    let rax: u64;
    let rcx: u64;
    unsafe {
        asm!("mov {}, rax", out(reg) rax);
        asm!("mov {}, rcx", out(reg) rcx);
    }

    error!("GPF DETAILS:");
    error!("- RIP: {:#x}", stack_frame.instruction_pointer);
    error!("- RAX: {:#x}", rax);
    error!("- RCX: {:#x}", rcx);
    error!("- Target Address: {:#x}", rax + rcx + 0xb0); // 计算目标地址
    error!("- Error Code: {:#x}", err_code);

    panic!("EXCEPTION: GENERAL PROTECTION FAULT");
}
