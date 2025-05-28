#![no_std]
#![no_main]

use ysos::*;
use ysos_kernel as ysos;

extern crate alloc;

boot::entry_point!(kernel_main);

pub fn kernel_main(boot_info: &'static boot::BootInfo) -> ! {
    ysos::init(boot_info);
    // FIXME: update lib.rs to pass following tests

    // 1. run some (about 5) "test", show these threads are running concurrently
    
    // 2. run "stack", create a huge stack, handle page fault properly

    let mut test_num = 0;

    loop {
        print!("[>] ");
        let line = input::get_line();
        match line.trim() {
            "exit" => break,
            "ps" => {
                ysos::proc::print_process_list();
            }
            "stack" => {
                ysos::new_stack_test_thread();
            }
            "test" => {
                ysos::new_test_thread(format!("{}", test_num).as_str());
                test_num += 1;
            }
            "app" => {
                ysos::proc::list_app();
            }
            _ => println!("[=] {}", line),
        }
    }
    ysos::shutdown();
}


// pub fn kernel_main(boot_info: &'static boot::BootInfo) -> ! {
//     ysos::init(boot_info);
//     ysos::wait(spawn_init());
//     ysos::shutdown();
// }

// pub fn spawn_init() -> proc::ProcessId {
//     // NOTE: you may want to clear the screen before starting the shell
//     // print!("\x1b[1;1H\x1b[2J");

//     proc::list_app();
//     proc::spawn("sh").unwrap()
// }
