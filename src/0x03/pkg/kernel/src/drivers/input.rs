use alloc::string::String;
use alloc::vec::Vec;
use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;
use log::warn;
use pc_keyboard::DecodedKey;

pub type Key = DecodedKey;
// 自定义缓冲区大小为128
const BUFFER_SIZE: usize = 128;

// 确保初始化（调试用）
lazy_static! {
    static ref INPUT_BUF: ArrayQueue<Key> = {
        let queue = ArrayQueue::new(BUFFER_SIZE);
        queue
    };
}

// 将键压入输入缓冲区
#[inline]
pub fn push_key(key: Key) {
    if INPUT_BUF.push(key).is_err() {
        warn!("Input buffer is full. Dropping key '{:?}'", key);
        panic!("Buffer overflow in debug mode");
    }
}

// 非阻塞地从缓冲区尝试取出一个键
#[inline]
pub fn try_pop_key() -> Option<Key> {
    INPUT_BUF.pop()
}

// 阻塞式地从缓冲区取出一个键（循环等待直到有数据）
pub fn pop_key() -> Key {
    loop {
        if let Some(key) = try_pop_key() {
            return key;
        }
    }
}

// 从缓冲区读取一行（阻塞直到遇到 '\n'）
pub fn get_line() -> String {
    // 使用with_capacity预分配空间 并设置为String类型
    let mut line = String::with_capacity(BUFFER_SIZE);
    loop {
        let key = pop_key();
        match key {
            DecodedKey::Unicode('\x08') | DecodedKey::Unicode('\x7F') => {
                if !line.is_empty() {
                    line.pop();
                    backspace();
                }
            }
            DecodedKey::Unicode('\n') | DecodedKey::Unicode('\r') => {
                println!();
                break;
            }
            DecodedKey::Unicode(c) => {
                if line.len() < BUFFER_SIZE {
                    line.push(c);
                    print!("{}", c);
                }
            }
            _ => continue,
        }
    }
    line
}

// 在终端执行退格操作
fn backspace() {
    // 发送退格序列：\x08 退格，\x20 空格，\x08 再退格
    print!("\x08");
    print!("\x20");
    print!("\x08");
}
