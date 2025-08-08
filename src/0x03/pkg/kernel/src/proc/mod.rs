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
//! - 进程切换： [`switch`], [`spawn_kernel_thread`], [`process_exit`]
//! - 状态查看： [`print_process_list`], [`env`]
//! - 错误处理： [`handle_page_fault`]
//!

mod context;
mod data;
pub mod manager;
mod paging;
mod pid;
mod process;
mod processor;
mod vm;

use manager::*;
use process::*;
use processor::*;
use vm::*;

use alloc::string::String;
pub use context::ProcessContext;
pub use data::ProcessData;
pub use paging::PageTableContext;
pub use pid::ProcessId;

use x86_64::VirtAddr;
use x86_64::structures::idt::PageFaultErrorCode;
/// Constant defination: kernel's pid is always 1
pub const KERNEL_PID: ProcessId = ProcessId(1);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ProgramStatus {
    Running,
    Ready,
    Blocked,
    Dead,
}

/// init process manager
pub fn init() {
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
    manager::init(kproc);

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

pub fn spawn_kernel_thread(entry: fn() -> !, name: String, data: Option<ProcessData>) -> ProcessId {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let entry = VirtAddr::new(entry as usize as u64);
        get_process_manager().spawn_kernel_thread(entry, name, data)
    })
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
