use crate::*;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::str::from_utf8;

pub struct Stdin;
pub struct Stdout;
pub struct Stderr;

const UTF8_MAX_SIZE: usize = 4;

impl Stdin {
    fn new() -> Self {
        Self
    }

    pub fn read_line(&self) -> String {
        // 1. allocate string
        let mut line = String::new();

        loop {
            // 2. read from input buffer
            //       - char by char?
            let buf: &mut [u8] = &mut [0u8; UTF8_MAX_SIZE];
            let ret = sys_read(0, buf);

            if ret.is_none() {
                break;
            } else {
                let count: usize = ret.unwrap();
                // 3. match and handle different keys with utf8 characters
                match from_utf8(&buf[..count]) {
                    Ok(s) => {
                        if !s.is_empty() {
                            let ch: char = s.chars().next().unwrap();
                            match ch {
                                // Handle backspace/delete (remove last characters and update terminal display)
                                '\x08' | '\x7F' => {
                                    if !line.is_empty() {
                                        line.pop();
                                    }
                                    sys_write(1, "\x08\x20\x08".as_bytes());
                                }
                                
                                // Handle newline (end of input)
                                '\n' | '\r' => {
                                    sys_write(1, "\n".as_bytes());
                                    return line;
                                }

                                // Handle printable characters
                                _ => {
                                    line.push(ch);
                                    sys_write(1, &buf[..count]);
                                }
                            }
                        } else {
                            continue;
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        // 4. return string
        line
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
