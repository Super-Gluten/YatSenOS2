use crate::drivers::input::try_pop_key;
use crate::interrupt::serial::UTF8_MAX_SIZE;
use alloc::collections::BTreeMap;
use alloc::string::String;
use pc_keyboard::DecodedKey;
use spin::Mutex;

#[derive(Debug, Clone)]
pub enum StdIO {
    Stdin,
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub struct ResourceSet {
    pub handles: BTreeMap<u8, Mutex<Resource>>,
}

impl Default for ResourceSet {
    fn default() -> Self {
        let mut res = Self {
            handles: BTreeMap::new(),
        };

        res.open(Resource::Console(StdIO::Stdin));
        res.open(Resource::Console(StdIO::Stdout));
        res.open(Resource::Console(StdIO::Stderr));

        res
    }
}

impl ResourceSet {
    pub fn open(&mut self, res: Resource) -> u8 {
        let fd = self.handles.len() as u8;
        self.handles.insert(fd, Mutex::new(res));
        fd
    }

    pub fn close(&mut self, fd: u8) -> bool {
        self.handles.remove(&fd).is_some()
    }

    pub fn read(&self, fd: u8, buf: &mut [u8]) -> isize {
        if let Some(count) = self.handles.get(&fd).and_then(|h| h.lock().read(buf)) {
            count as isize
        } else {
            -1
        }
    }

    pub fn write(&self, fd: u8, buf: &[u8]) -> isize {
        if let Some(count) = self.handles.get(&fd).and_then(|h| h.lock().write(buf)) {
            count as isize
        } else {
            -1
        }
    }
}

#[derive(Debug)]
pub enum Resource {
    Console(StdIO),
    Null,
}

impl Resource {
    /// differentiated data reading based on `Resource` filed
    ///
    /// # Returns
    /// - Some(count) : Number of bytes read
    /// - None: 
    ///   - When read from `Stdout`, `Stderr`.
    pub fn read(&mut self, buf: &mut [u8]) -> Option<usize> {
        match self {
            Resource::Console(stdio) => match stdio {
                StdIO::Stdin => {
                    // 1. ensure `buf` can obtain complete UTF8 sequence
                    if buf.len() < UTF8_MAX_SIZE {
                        Some(0)
                    } else {
                        // 2. just read from kernel input buffer
                        //      without blocking
                        match try_pop_key() {
                            // 3. avoid special characters
                            Some(DecodedKey::Unicode(key)) => {
                                let s = key.encode_utf8(buf);
                                Some(s.len())
                            }
                            _ => Some(0),
                        }
                    }
                }
                _ => None,
            },
            Resource::Null => Some(0),
        }
    }

    /// differentiated data reading based on `Resource` filed
    pub fn write(&mut self, buf: &[u8]) -> Option<usize> {
        match self {
            Resource::Console(stdio) => match *stdio {
                StdIO::Stdin => None,
                StdIO::Stdout => {
                    print!("{}", String::from_utf8_lossy(buf));
                    Some(buf.len())
                }
                StdIO::Stderr => {
                    warn!("{}", String::from_utf8_lossy(buf));
                    Some(buf.len())
                }
            },
            Resource::Null => Some(buf.len()),
        }
    }
}
