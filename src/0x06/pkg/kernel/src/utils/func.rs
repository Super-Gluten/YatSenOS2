use alloc::format;
use alloc::string::String;
use lazy_static::lazy_static;
use spin::Mutex;

lazy_static! {
    static ref STRING: Mutex<String> = Mutex::new(String::new());
}

const MAX_STRING_LENGTH: usize = 100;

pub fn test() -> ! {
    let mut count = 0;
    let id;
    if let Some(id_env) = crate::proc::env("id") {
        id = id_env
    } else {
        id = "unknown".into()
    }
    loop {
        // display processes switching by printing ID
        count += 1;
        if count == 10000 {
            count = 0;
            let mut string = STRING.lock();
            let add_str = format!("{} ", id);

            if string.len() > MAX_STRING_LENGTH {
                println!();
                string.clear();
            }

            string.push_str(&add_str);
            print!("{}", add_str);
        }

        x86_64::instructions::hlt();
    }
}

#[inline(never)]
fn huge_stack() {
    println!("Huge stack testing...");

    let mut stack = [0u64; 0x1000];

    for (idx, item) in stack.iter_mut().enumerate() {
        *item = idx as u64;
    }

    for i in 0..stack.len() / 256 {
        println!("{:#05x} == {:#05x}", i * 256, stack[i * 256]);
    }
}

pub fn stack_test() -> ! {
    huge_stack();
    crate::proc::process_exit(0)
}
