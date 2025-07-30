use alloc::vec::Vec;
use core::str::from_utf8;
use pc_keyboard::DecodedKey;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use super::consts::*;
use crate::drivers::{
    input::push_key,
    serial::get_serial_for_sure,
};

/// Maxmium bytes of UTF-8 characters
const SERIAL_BUFFER_SIZE: usize = 4;

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
    let mut serial_buffer: Vec<u8> = Vec::with_capacity(SERIAL_BUFFER_SIZE);
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
                            let ch = s.chars().next().unwrap();
                            push_key(DecodedKey::Unicode(ch));
                            serial_buffer.clear();
                        }
                    }
                    Err(_) => {
                        // otherwise, clear the SERIAL_BUFFER if overflow
                        if serial_buffer.len() >= SERIAL_BUFFER_SIZE {
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
