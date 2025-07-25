/// initialize the serial and print the sign

use super::uart16550::SerialPort;

const SERIAL_IO_PORT: u16 = 0x3F8; // COM1

once_mutex!(pub SERIAL: SerialPort);

pub fn init() {
    init_SERIAL(SerialPort::new(SERIAL_IO_PORT));
    get_serial_for_sure().init();

    // escape sequence and print the sign
    println!("\x1B[2J\x1B[H");
    let student_num = "xxxxxxxx xxx";
    println!("{}{}{}", "\x1b[36m", student_num, "\x1b[0m");
    println!("{}", crate::get_ascii_header());
    println!("[+] Serial Initialized.");
}

guard_access_fn!(pub get_serial(SERIAL: SerialPort));
