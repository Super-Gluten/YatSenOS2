#![no_std]
#![allow(dead_code)]
#![feature(naked_functions)]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(type_alias_impl_trait)]
#![feature(map_try_insert)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::result_unit_err)]

extern crate alloc;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate bitflags;
extern crate libm;

#[macro_use]
pub mod utils;
pub use utils::*;

#[macro_use]
pub mod drivers;
pub use drivers::*;

pub mod interrupt;
pub mod memory;
pub mod proc;

pub use alloc::format;

use boot::BootInfo;
use uefi::{Status, runtime::ResetType};

pub fn init(boot_info: &'static BootInfo) {
    unsafe {
        // set uefi system table
        uefi::table::set_system_table(boot_info.system_table.cast().as_ptr());
    }

    drivers::serial::init(); // init serial output
    logger::init("info"); // init logger system

    info!("YatSenOS initialized.");

    // 0x07 add: 栈扩容成果测试
    info!("Test stack grow.");

    grow_stack();

    info!("Stack grow test done.");
}

pub fn shutdown() -> ! {
    info!("YatSenOS shutting down.");
    uefi::runtime::reset(ResetType::SHUTDOWN, Status::SUCCESS, None);
}

pub fn wait(init: proc::ProcessId) {
    info!("wait happen");
    loop {
        if proc::still_alive(init) {
            // Why? Check reflection question 5
            x86_64::instructions::hlt();
        } else {
            break;
        }
    }
} // 0x04 add

// 0x07 add: 栈扩容测试函数
#[inline(never)]
#[unsafe(no_mangle)]
pub fn grow_stack() {
    const STACK_SIZE: usize = 1024 * 4;
    const STEP: usize = 64;

    let mut array = [0u64; STACK_SIZE];
    info!("Stack: {:?}", array.as_ptr());

    // test write
    for i in (0..STACK_SIZE).step_by(STEP) {
        array[i] = i as u64;
    }

    // test read
    for i in (0..STACK_SIZE).step_by(STEP) {
        assert_eq!(array[i], i as u64);
    }
}