mod context;
mod data;
pub mod manager; // 因为在util/mod.rs中引用了proc::*，且需要manager
mod paging;
mod pid;
mod process;
pub mod processor;
mod vm;

use manager::*;
use process::*;
use vm::*;
use processor::*; // 在switch函数中使用了proceeor相关的函数
use crate::memory::PAGE_SIZE;

use alloc::string::String;
pub use context::ProcessContext;
pub use paging::PageTableContext;
pub use data::ProcessData;
pub use pid::ProcessId;
pub use manager::ProcessManager;

use x86_64::structures::idt::PageFaultErrorCode;
use x86_64::VirtAddr;
pub const KERNEL_PID: ProcessId = ProcessId(1); // 常量定义：内核进程pid为1

use alloc::vec::Vec;
use alloc::format;
use alloc::string::ToString;
use alloc::sync::{Arc, Weak};
use xmas_elf::ElfFile;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProgramStatus {
    Running,
    Ready,
    Blocked,
    Dead,
}

/// init process manager
pub fn init(boot_info: &'static boot::BootInfo) {  // 0x04 add parameter
    let proc_vm = ProcessVm::new(PageTableContext::new()).init_kernel_vm();

    trace!("Init kernel vm: {:#?}", proc_vm);

    // kernel process
    let kproc = { /* FIXME: create kernel process */ 
        Process::new(
            "kernel".into(),
            None,
            Some(proc_vm), // 使用已经建好的proc_vm就好
            Some(ProcessData::new())
        )
    };

    // 0x04 add :
    let app_list = boot_info.loaded_apps.as_ref();
    manager::init(kproc, app_list);

    info!("Process Manager Initialized.");
}

pub fn switch(context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // FIXME: switch to the next process
        //      - save current process's context
        let manager = get_process_manager();
        manager.save_current(context);

        //      - handle ready queue update
        manager.push_ready(get_pid());

        //      - restore next process's context
        manager.switch_next(context);
        // 三个相关的函数功能见manager.rs对应函数
    });
}

// pub fn spawn_kernel_thread(entry: fn() -> !, name: String, data: Option<ProcessData>) -> ProcessId {
//     x86_64::instructions::interrupts::without_interrupts(|| {
//         let entry = VirtAddr::new(entry as usize as u64);
//         get_process_manager().spawn_kernel_thread(entry, name, data)
//     })
// } // 0x03 add, 0x04 delete

pub fn print_process_list() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().print_process_list();
    })
}

pub fn env(key: &str) -> Option<String> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // FIXME: get current process's environment variable
        get_process_manager().current().read().env(key)
        // Process的.read()返回 ProcessInner.read()，然后通过deref方法解引用为ProcessData
        // 最后使用ProcessData中定义的方法env
    })
}

// pub fn process_exit(ret: isize) -> ! {
//     x86_64::instructions::interrupts::without_interrupts(|| {
//         get_process_manager().kill_current(ret);
//         info!("done killing");
//     });

//     loop {
//         x86_64::instructions::hlt();
//     }
// } // 0x04 delete && update:

pub fn process_exit(ret: isize, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        // FIXME: implement this for ProcessManager
        manager.kill_current(ret);
        manager.switch_next(context);
    })
}

pub fn handle_page_fault(addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().handle_page_fault(addr, err_code)
    })
}

pub fn list_app() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let app_list = get_process_manager().app_list();
        if app_list.is_none() {
            println!("[!] No app found in list!");
            return;
        }

        // let apps = app_list
        //     .unwrap() // 取出Option<>中的'static AppListRef
        //     .iter() // 遍历array<>中每一个App元素
        //     .map(|app| app.name.as_str()) // 将其中App元素映射为它的名字，转为&str
        //     .collect::<Vec<&str>>() // 这个str加入Vec<&str>的末尾
        //     .join(", "); // 元素之间采用','间隔开
        // println!("[+] App list: {}", apps);

        // TODO: print more information like size, entry point, etc.
        println!("[+] App list ({} applications):", app_list.unwrap().len()); // 打印总共有多少个应用
        for app in app_list.unwrap().iter() {
            let elf = &app.elf;
            println!(
                "{}: {} , {}",
                app.name,
                format!("{} kb", elf.input.len()/1024 ), // 通过elf文件的读取长度计算实际大小
                elf.header.pt2.entry_point() // elf头文件有入口点
            );
        }
        
    });
} // 0x04：用于列出当前系统中的所有用户程序和相关信息


// 0x04 add: spawn && elf_spawn && read && write
pub fn spawn(name: &str) -> Option<ProcessId> {
    let app = x86_64::instructions::interrupts::without_interrupts(|| {
        let app_list = get_process_manager().app_list()?;
        app_list.iter().find(|&app| app.name.eq(name))
    })?;

    elf_spawn(name.to_string(), &app.elf)
}

pub fn elf_spawn(name: String, elf: &ElfFile) -> Option<ProcessId> {
    let pid = x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let process_name = name.to_lowercase();
        let parent = Arc::downgrade(&manager.current());
        let pid = manager.spawn(elf, name, Some(parent), None);

        debug!("Spawned process: {}#{}", process_name, pid);
        pid
    });

    Some(pid)
}

pub fn read(fd: u8, buf: &mut [u8]) -> isize {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().read(fd, buf))
}

pub fn write(fd: u8, buf: &[u8]) -> isize {
    x86_64::instructions::interrupts::without_interrupts(|| get_process_manager().write(fd, buf))
}

#[inline]
pub fn still_alive(pid: ProcessId) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // check if the process is still alive
        match get_process_manager().get_proc(&pid){
            Some(proc) => proc.read().is_dead(),
            None => return false,
        }
    })
}

pub fn wait_pid(pid: ProcessId, context: &mut ProcessContext) -> isize {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        match manager.get_proc(&pid) {
            Some(proc) => {
                if proc.read().is_dead() {
                    return 0;
                } else {
                    let ret = proc.read().exit_code().unwrap();
                    process_exit(ret, context);
                    return ret;
                }
            },
            None => return 0,
        }
    })
}