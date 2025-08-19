//! 进程管理模块
//!
//! 该模块提供了操作系统核心的进程管理功能，包括：
//! - 进程创建、调度和销毁
//! - 进程上下文切换
//! - 虚拟内存管理（用户栈、内核栈、页表映射）
//! - 内核线程管理
//!
//! # 主要组件
//! - **进程管理**：
//!   - [`manager`] 定义 ProcessManager，维护就绪队列与进程键值对
//!   - [`processor`] 定义 Processor，当前CPU的运行进程记录器
//! - **进程结构与数据**：
//!   - [`process`] 定义 Process，提供与进程生命周期相关的方法
//!   - [`pid`] 定义 ProcessId，记录进程的特定id
//!   - [`context`] 定义 ProcessContext，记录寄存器堆信息和中断栈值
//!   - [`data`] 定义 ProcessData，记录环境信息
//!   - [`paging`] 定义 PageTableContext，记录页表信息
//! - **虚拟内存管理**：
//!   - [`vm`] 提供进程虚拟内存管理，包括用户栈、内核栈和内存映射
//!
//! # 主要方法
//! - 管理器初始化： [`init`]
//! - 进程切换： [`switch`], [`process_exit`]
//! - 状态查看： [`print_process_list`], [`env`], [`list_app`]
//! - 错误处理： [`handle_page_fault`]
//!

mod context;
mod data;
pub mod manager;
mod paging;
mod pid;
mod process;
pub mod processor;
mod vm;

use manager::*;
use process::*;
use processor::*;
use vm::*;

use alloc::string::String;
pub use context::ProcessContext;
use core::fmt;
pub use data::ProcessData;
pub use paging::PageTableContext;
pub use pid::ProcessId;

use x86_64::VirtAddr;
use x86_64::structures::idt::PageFaultErrorCode;
/// Constant defination: kernel's pid is always 1
pub const KERNEL_PID: ProcessId = ProcessId(1);

use alloc::format;
use alloc::string::ToString;
use alloc::sync::Arc;
use xmas_elf::ElfFile;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProgramStatus {
    Running,
    Ready,
    Blocked,
    Dead,
}

/// init process manager
pub fn init(boot_info: &'static boot::BootInfo) {
    let proc_vm = ProcessVm::new(PageTableContext::new()).init_kernel_vm();

    trace!("Init kernel vm: {:#?}", proc_vm);

    // kernel process
    let kproc = {
        Process::new(
            "kernel".into(),
            None,
            Some(proc_vm),
            Some(ProcessData::new()),
        )
    };

    let app_list = boot_info.loaded_apps.as_ref();
    manager::init(kproc, app_list);

    info!("Process Manager Initialized.");
}

pub fn switch(context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        //       switch to the next process
        //      - save current process's context
        let manager = get_process_manager();
        manager.save_current(context);

        //      - handle ready queue update
        manager.push_ready(get_pid());

        //      - restore next process's context
        manager.switch_next(context);
    });
}

pub fn print_process_list() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().print_process_list();
    })
}

pub fn env(key: &str) -> Option<String> {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // get current process's environment variable
        get_process_manager().current().read().env(key)
    })
}

pub fn process_exit(ret: isize) -> ! {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().kill_current(ret);
        info!("done killing");
    });

    loop {
        x86_64::instructions::hlt();
    }
}

pub fn handle_page_fault(addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| {
        get_process_manager().handle_page_fault(addr, err_code)
    })
}

pub fn list_app() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // list all app and related information
        let app_list = get_process_manager().app_list();
        if app_list.is_none() {
            println!("[!] No app found in list!");
            return;
        }

        // print the number of application
        println!("[+] App list ({} applications):", app_list.unwrap().len());
        println!(
            "{:<2} | {:<10} | {:<8} | {}",
            "#", "Name", "Size", "Entry_point"
        );
        // print app information including name, size and entry point.
        for (index, app) in app_list.unwrap().iter().enumerate() {
            let elf = &app.elf;
            println!(
                "{:<2} | {:<10} | {:<8} | {:#x}",
                format!("{} ", index + 1),
                app.name,
                format!("{} kb", elf.input.len() / 1024), // 通过elf文件的读取长度计算实际大小
                elf.header.pt2.entry_point()              // elf头文件有入口点
            );
        }
    });
}

pub fn spawn(name: &str) -> Option<ProcessId> {
    let app = x86_64::instructions::interrupts::without_interrupts(|| {
        // find the corrsponding app by name and spawn it
        let app_list = get_process_manager().app_list()?;
        app_list.iter().find(|&app| app.name.eq(name))
    })?;

    elf_spawn(name.to_string(), &app.elf)
}

pub fn elf_spawn(name: String, elf: &ElfFile) -> Option<ProcessId> {
    let pid = x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let process_name: String = name.to_lowercase();
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

pub fn exit(ret: isize, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        manager.kill_current(ret);
        manager.switch_next(context);
    })
}

#[inline]
pub fn still_alive(pid: ProcessId) -> bool {
    x86_64::instructions::interrupts::without_interrupts(|| {
        // check if the process is still alive
        match get_process_manager().get_proc(&pid) {
            Some(proc) => !proc.read().is_dead(),
            None => return false,
        }
    })
}

pub fn wait_pid(pid: ProcessId, context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        let proc = manager.get_proc(&pid).unwrap();
        if !still_alive(pid) {
            let exit_code: isize = proc.read().exit_code().unwrap();
            context.set_rax(exit_code as usize);
            manager.save_current(context);
            manager.switch_next(context);
        }
    });
}

pub fn fork(context: &mut ProcessContext) {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let manager = get_process_manager();
        // 1. save_current as parent
        manager.save_current(&context);
        // 2. fork to get child
        let child = manager.fork();
        // 3. push to child & parent to ready queue
        manager.push_ready(child.pid());
        manager.push_ready(manager.current().pid());
        // 4. switch to next process
        manager.switch_next(context);
    })
}

impl fmt::Display for ProgramStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status_str: &'static str = match self {
            ProgramStatus::Running => "Running",
            ProgramStatus::Ready => "Ready  ",
            ProgramStatus::Blocked => "Blocked",
            ProgramStatus::Dead => "Dead   ",
        };
        write!(f, "{}", status_str)
    }
}
