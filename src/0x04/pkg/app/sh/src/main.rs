#![no_std]
#![no_main]

mod consts;

extern crate lib;
use lib::*;
use consts::*;
use alloc::string::{String, ToString};


fn main() -> isize {
    print!("\x1B[2J\x1B[H"); // 清屏

    println!("\n\n");

    // 组合字母形成完整的banner
    let mut banner = [
        String::from(SIGN[0]),
        String::from(SIGN[1]),
        String::from(SIGN[2]),
        String::from(SIGN[3]),
        String::from(SIGN[4]),
        STUDENT_INFO.to_string(),
    ];

    for (i, line) in banner.iter().enumerate() {
        println!("{RESET}");

        print!("\x1B[1A\x1B[2C");
        let color = RAINBOW[i % RAINBOW.len()];
        println!("{BOLD}{color}{}{RESET}", line);
    }

    loop {
        print!("{BOLD}{R3}[YatSenOS]{R4}> {RESET}");
        let binding = stdin().read_line();
        let mut command = binding.trim().split(' '); // 去除首尾的空白字符，并按空格分隔命令和参数
        let op = command.next().unwrap(); // 第一个单词是命令op
        println!("info!: information is: {}", op);

        match op {
            "help" => {
                println!("\n====可用命令列表为====\n");

                let commands = [
                    ("la", "列出所有可用应用"),
                    ("run <路径>", "运行指定路径的应用程序"),
                    ("ps", "显示系统状态"),
                    ("clear", "清屏"),
                    ("exit", "退出终端"),
                ];

                for (idx, (cmd, cmd_info)) in commands.iter().enumerate() {
                    println!("{}: {}  -->  {}", idx, cmd, cmd_info);
                }
                println!(
                    "any question can ask inventor with information\n{}",
                    STUDENT_INFO,
                );
            }
            "la" => {
                sys_list_app();
            }
            "run" => match command.next() {
                Some(path) => {
                    let name: vec::Vec<&str> = path.rsplit('/').collect();
                    let pid = sys_spawn(path);
                    if pid == 0 {
                        println!("Failed to run app: {}", name[0]);
                        continue;
                    } else {
                        sys_stat();
                        println!("exited with {}: {}", name[0], sys_wait_pid(pid));
                        // sys_sleep();
                    }
                }
                None => println!("Error: Please specify application path"),
            },
            "ps" => {
                println!("=====系统状态=====");
                sys_stat();
            }
            "exit" => {
                let goodbye = "Goodbye! See you next time!";
                println!("{}", goodbye);
                break;
            }
            "clear" => {
                print!("\x1B[2J\x1B[H"); // 完成清屏
            }
            "" => {}  // 处理空输入
            _ => {
                println!("Unknown command: {}; maybe you can try 'help' command?", op);
            }
        }
    }
    0
}

entry!(main);
