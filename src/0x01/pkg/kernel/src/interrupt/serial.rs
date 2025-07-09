use alloc::vec::Vec;
use core::str::from_utf8;
use pc_keyboard::DecodedKey;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use super::consts::*;
use crate::drivers::{
    input::{Key, push_key},
    serial::get_serial_for_sure
};

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    // 注册串口输出中断处理程序
    // 依然采用偏移的方式计算中断向量号，这里对应的应该是36
    idt[Interrupts::IrqBase as u8 + Irq::Serial0 as u8].set_handler_fn(serial_handler);
}

pub extern "x86-interrupt" fn serial_handler(_st: InterruptStackFrame) {
    receive();
    // 通知中断控制器->中断处理完成
    super::ack();
}

// 由于UTF-8编码字符的最大字节长度为4字节，那么定义缓冲区最大大小为4字节
const INPUT_BUFFER_SIZE: usize = 4;

/// Receive character from uart 16550
/// Should be called on every interrupt
fn receive() {
    // FIXME: receive character from uart 16550, put it into INPUT_BUFFER
    let mut input_buffer: Vec<u8> = Vec::with_capacity(INPUT_BUFFER_SIZE);
    loop {
        let mut serial = get_serial_for_sure(); // 获取串口实例
        // 使用uart16550.rs中的receive() 尝试从串口读一个字节
        let rec = serial.receive();
        drop(serial); // 显式释放串口资源

        match rec {
            Some(c) => {
                // 在receive()中尝试读取成功则压入缓冲区
                input_buffer.push(c);
                match from_utf8(&input_buffer) {
                    // 成功解析为UTF-8序列
                    Ok(s) => {
                        if !s.is_empty() {
                            let ch = s.chars().next().unwrap();
                            push_key(DecodedKey::Unicode(ch));
                            input_buffer.clear();
                        } 
                    },
                    Err(_) => if input_buffer.len() >= INPUT_BUFFER_SIZE {
                        input_buffer.clear();
                        info!("无效的UTF-8序列，已清空缓冲区");
                    }
                }
            },
            _ => {
                break; // 其他情况下继续累积字节，可能是不完整的UTF-8序列
            }
        }
    }
}
