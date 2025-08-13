use alloc::vec::Vec;
use core::str::from_utf8;
use pc_keyboard::DecodedKey;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use super::consts::*;
use crate::drivers::{input::push_key, serial::get_serial_for_sure};

/// Maxmium bytes of UTF-8 characters
pub const UTF8_MAX_SIZE: usize = 4;

/// Register serial interrupt handler
pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    idt[SERIAL_INTERRUPT_VECTOR].set_handler_fn(serial_handler);
}

pub extern "x86-interrupt" fn serial_handler(_st: InterruptStackFrame) {
    receive();
    super::ack();
}

/// Receive character from uart 16550
/// Should be called on every interrupt
fn receive() {
    // receive character from uart 16550, put it into SERIAL_BUFFER
    let mut serial_buffer: Vec<u8> = Vec::with_capacity(UTF8_MAX_SIZE);
    loop {
        // access serial port through mutex lock
        let mut serial = get_serial_for_sure();
        let rec = serial.receive();
        drop(serial);

        match rec {
            Some(c) => {
                serial_buffer.push(c); // press into buffer if successfully read
                match from_utf8(&serial_buffer) {
                    Ok(s) => {
                        // push key to input_buffer if successfully parsed
                        if !s.is_empty() {
                            // the size of `serial_buffer` is less than a char
                            // that's why `ch` can be converted into the parameter of `push_key()` 
                            let ch = s.chars().next().unwrap();
                            push_key(DecodedKey::Unicode(ch));
                            serial_buffer.clear();
                        }
                    }
                    Err(_) => {
                        // otherwise, clear the UTF8_MAX_SIZE if overflow
                        if serial_buffer.len() >= UTF8_MAX_SIZE {
                            serial_buffer.clear();
                            info!("无效的UTF-8序列，已清空缓冲区");
                        }
                    }
                }
            }
            _ => {
                break; // otherwise accumlate bytes, probably incomplete utf-8 sequence
            }
        }
    }
}
