use alloc::string::String;
use crossbeam_queue::ArrayQueue;
use lazy_static::lazy_static;
use log::warn;
use pc_keyboard::DecodedKey;

/// Type alias for key board input events
///
/// # Explain
/// pub enum DecodedKey {
///     Unicode(char),
///     RawKey(KeyCode),
/// }

pub type Key = DecodedKey;
/// Size of the input buffer (in number of key events)
const BUFFER_SIZE: usize = 128;

lazy_static! {
    /// lazy initialization of the global input buffer
    static ref INPUT_BUF: ArrayQueue<Key> = {
        let queue = ArrayQueue::new(BUFFER_SIZE);
        queue
    };
}

/// Push a key event info the input buffer
#[inline]
pub fn push_key(key: Key) {
    if INPUT_BUF.push(key).is_err() {
        warn!("Input buffer is full. Dropping key '{:?}'", key);
        panic!("Buffer overflow in debug mode");
    }
}

/// Try to pop a key event from input buffer (non-blocking)
pub fn try_pop_key() -> Option<Key> {
    INPUT_BUF.pop()
}

/// Pop a key event from input buffer (blocking)
pub fn pop_key() -> Key {
    loop {
        if let Some(key) = try_pop_key() {
            return key;
        }
    }
}

/// Read a line of input from the keyboard buffer
///
/// # Returns
/// The collected line of text as a string
pub fn get_line() -> String {
    let mut line = String::with_capacity(BUFFER_SIZE);
    loop {
        // debug_buffer();
        let key = pop_key();

        match key {
            // Handle backspace/delete (remove last characters and update terminal display)
            DecodedKey::Unicode('\x08') | DecodedKey::Unicode('\x7F') => {
                if !line.is_empty() {
                    line.pop();
                    backspace();
                }
            }
            // Handle newline (end of input)
            DecodedKey::Unicode('\n') | DecodedKey::Unicode('\r') => {
                println!();
                break;
            }

            // Handle printable characters
            DecodedKey::Unicode(c) => {
                if line.len() < BUFFER_SIZE {
                    line.push(c);
                    print!("{}", c);
                }
            }

            _ => continue, // Ignore non-Unicode keys
        }
    }
    line
}

/// Perform a backspace operation in the terminal
fn backspace() {
    print!("\x08");
    print!("\x20");
    print!("\x08");
}
