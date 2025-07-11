use alloc::string::String;
use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;
use log::warn;
// pc_keyboard：用于处理PC键盘输入的rust库
use pc_keyboard::DecodedKey;
use alloc::vec::Vec;

// 输入类型定义
// pub enum DecodedKey {
//     Unicode(char), // 已解码的Unicode字符
//     RawKey(KeyCode), // 无法映射为Unicode的特殊键
// }
pub type Key = DecodedKey;
// 自定义内核缓冲区大小为128
const BUFFER_SIZE: usize = 128;

// 确保初始化（调试用）
lazy_static! {
    static ref INPUT_BUF: ArrayQueue<Key> = {
        let queue = ArrayQueue::new(BUFFER_SIZE);
        // 正确获取地址的方式：
        // println!("INPUT_BUF address: {:p}", &queue);
        queue
    };
}

// 将键压入输入缓冲区
#[inline]
pub fn push_key(key: Key) {
    if INPUT_BUF.push(key).is_err() {
        warn!("Input buffer is full. Dropping key '{:?}'", key);
        // panic!("Buffer overflow in debug mode");
    }
}

// 非阻塞地从缓冲区尝试取出一个键
#[inline]
pub fn try_pop_key() -> Option<Key> {
    INPUT_BUF.pop()
}

// 阻塞式地从缓冲区取出一个键（循环等待直到有数据）
// 注意需要将pop_key暴露出来，所以是pub
// 调用ArrayQueue自带的pop函数
pub fn pop_key() -> Key {
    loop {
        if let Some(key) = try_pop_key() {
            // 要返回获取到的数据
            return key;
        }
    }
}

// pub fn debug_buffer() {
//     println!("Buffer capacity: {}", INPUT_BUF.capacity());
//     println!("Buffer len: {}", INPUT_BUF.len());
//     // 注意：ArrayQueue 不提供直接迭代的方法
//     // 可以通过临时弹出所有元素来检查内容
//     let mut temp = Vec::new();
//     while let Some(key) = INPUT_BUF.pop() {
//         println!("  Contains: {:?}", key);
//         temp.push(key);
//     }
//     // 将元素放回队列
//     for key in temp {
//         INPUT_BUF.push(key).unwrap();
//     }
// }

// 从缓冲区读取一行（阻塞直到遇到 '\n'）
pub fn get_line() -> String {
    // 使用with_capacity预分配空间 并设置为String类型
    let mut line = String::with_capacity(BUFFER_SIZE);
    loop {
        // debug_buffer();
        let key = pop_key();

        match key {
            // 处理退格
            DecodedKey::Unicode('\x08') | DecodedKey::Unicode('\x7F') => {
                if !line.is_empty() {
                    line.pop(); // 移除最后一个字符
                    // 封装为backspace()函数
                    backspace();
                }
            }
            // 换行符结束输入
            DecodedKey::Unicode('\n') | DecodedKey::Unicode('\r') => {
                println!(); // 换行
                break;
            }

            // 可打印字符
            DecodedKey::Unicode(c) => {
                // 做缓冲区的溢出防护
                if line.len() < BUFFER_SIZE {
                    line.push(c);
                    print!("{}", c);
                }
            }

            _ => continue, // 忽略非Unicode键
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
