#![no_std]
#![no_main]

extern crate lib;
use lib::*;

fn main() -> isize {
    print!("\x1B[2J\x1B[H"); // 清屏

    const RESET: &str = "\x1b[0m";
    const BOLD: &str = "\x1b[1m";
    const BLINK: &str = "\x1b[5m";
    const DIM: &str = "\x1b[2m"; // 暗淡效果，用于制造阴影
    const R1: &str = "\x1b[91m"; // 亮红
    const R2: &str = "\x1b[93m"; // 亮黄
    const R3: &str = "\x1b[92m"; // 亮绿
    const R4: &str = "\x1b[96m"; // 亮青
    const R5: &str = "\x1b[94m"; // 亮蓝
    const R6: &str = "\x1b[95m"; // 亮洋红
    const RAINBOW: [&str; 6] = [R1, R2, R3, R4, R5, R6];

    println!("\n\n");

    let banner = [
        " __   __      _  _____ ____  _____ _   _    ___  ____  ",
        " \\ \\ / /___ _| ||_   _/ ___|| ____| \\ | |  / _ \\/ ___| ",
        "  \\ V // _` | __|| | \\___ \\|  _| |  \\| | | | | \\___ \\ ",
        "   | || (_| | |_ | |  ___) | |___| |\\  | | |_| |___) |",
        "   |_| \\__,_|\\__||_| |____/|_____|_| \\_|  \\___/|____/ ",
        "    学号：23336345  姓名： 周海铭",
    ];

    for (i, line) in banner.iter().enumerate() {
        print!("   {DIM}");
        for ch in line.chars() {
            if ch != ' ' {
                print!("█");
            } else {
                print!(" ");
            }
        }
        println!("{RESET}");

        print!("\x1B[1A\x1B[2C");
        let color = RAINBOW[i % RAINBOW.len()];
        println!("{BOLD}{color}{}{RESET}", line);
    }
    let student_number = " 学号:23336345  姓名： 周海铭";

    loop {
        print!("{BOLD}{R3}[YatSenOS]{R4}> {RESET}");
        let binding = stdin().read_line();
        let mut command = binding.trim().split(' '); // 去除首尾的空白字符，并按空格分隔命令和参数
        let op = command.next().unwrap(); // 第一个单词是命令op

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
                    student_number
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
                break;
            }
            "clear" => {
                print!("\x1B[2J\x1B[H"); // 完成清屏
            }
            _ => {
                println!("Unknown command: {}; maybe you can try 'help' command?", op);
            }
        }
    }
    0
}

entry!(main);
