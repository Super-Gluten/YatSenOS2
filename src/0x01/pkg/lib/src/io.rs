use crate::*;
use alloc::string::{String, ToString};
use alloc::vec;

pub struct Stdin;
pub struct Stdout;
pub struct Stderr;

impl Stdin {
    fn new() -> Self {
        Self
    }

    pub fn read_line(&self) -> String {
        // FIXME: allocate string
        let mut line = String::new();
        // FIXME: read from input buffer
        //       - maybe char by char?
        // FIXME: handle backspace / enter...
        // FIXME: return string

        loop {
            let buf: &mut [u8] = &mut [0u8; 256];
            let ret = sys_read(0, buf);

            if ret.is_none() {
                continue;
            } else {
                for i in 0..ret.unwrap() {
                    let ch = buf[i];
                    match ch {
                        b'\r' => {
                            sys_write(1, "\n".as_bytes()); // 写入一个换行符
                            return line;
                        }
                        b'\x08' | b'\x7f' => {
                            line.pop();
                            sys_write(1, "\x08\x20\x08".as_bytes()); // 写入一个退格
                        }
                        _ => {
                            line.push(ch as char);
                            sys_write(1, &mut[ch]); // 写入一个字符ch
                        }
                    };
                }
            }
        }

        String::new()
    }
}

impl Stdout {
    fn new() -> Self {
        Self
    }

    pub fn write(&self, s: &str) {
        sys_write(1, s.as_bytes());
    }
}

impl Stderr {
    fn new() -> Self {
        Self
    }

    pub fn write(&self, s: &str) {
        sys_write(2, s.as_bytes());
    }
}

pub fn stdin() -> Stdin {
    Stdin::new()
}

pub fn stdout() -> Stdout {
    Stdout::new()
}

pub fn stderr() -> Stderr {
    Stderr::new()
}
