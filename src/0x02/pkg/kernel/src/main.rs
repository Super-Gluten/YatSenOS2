#![no_std]
#![no_main]

#[macro_use]
extern crate log;

use crate::drivers::input;
use crate::interrupt::clock::read_counter;
use ysos::*;
use ysos_kernel as ysos;

extern crate alloc;

boot::entry_point!(kernel_main);

pub fn kernel_main(boot_info: &'static boot::BootInfo) -> ! {
    ysos::init(boot_info);

    loop {
        print!("> ");
        let input = input::get_line();

        match input.trim() {
            "exit" => break,
            _ => {
                println!("you saids: {}", input);
                println!("The value of counter is {}", read_counter());
                println!("print \"exit\" to shutdown");
            }
        }
    }

    ysos::shutdown();
}
