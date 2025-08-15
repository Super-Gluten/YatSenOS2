#![no_std]
#![no_main]

use lib::*;

extern crate lib;

fn main() -> isize {
    println!("Hello, world!!!");
    // // [bot..0xffffff0100000000..top..0xffffff01ffffffff]
    // // kernel stack
    // let kernel_ask: u8;
    // unsafe {
    //     kernel_ask = *(0xffff_ff01_0000_0010 as *const u8);
    // }
    // println!("kernel_ask = {}", kernel_ask);
    233
}

entry!(main);
