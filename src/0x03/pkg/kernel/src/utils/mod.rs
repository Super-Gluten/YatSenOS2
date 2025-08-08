//! 实用工具模块
//!
//! 提供跨模块使用的核心工具功能，包括：
//! - 内核日志系统初始化与管理
//! - 寄存器操作宏和函数
//! - 常用宏定义
//! - 测试线程快速创建
//!
//! # 主要组件
//! - **日志系统**:
//!   - [`logger`] 根据引导程序日志等级设置，完成对应内核日志的输出
//! - **宏定义**:
//!   - [`macros`] 定义常用的互斥宏，打印宏，panic宏
//! - **寄存器操作**:
//!   - [`regs`] 定义 RegistersValue, 封装计算机寄存器堆
//!
//! # 主要方法
//! - 线程测试: [`new_test_thread`], [`new_stack_thread`]
//! - 单位转换: [`humanized_size`], [`humanized_size_short`]
//!

#[macro_use]
mod macros;
#[macro_use]
pub mod regs;

use alloc::format;
pub mod func;
pub mod logger;

pub use macros::*;
pub use regs::*;

use crate::proc::*;

pub const fn get_ascii_header() -> &'static str {
    concat!(
        r"
__  __      __  _____            ____  _____
\ \/ /___ _/ /_/ ___/___  ____  / __ \/ ___/
 \  / __ `/ __/\__ \/ _ \/ __ \/ / / /\__ \
 / / /_/ / /_ ___/ /  __/ / / / /_/ /___/ /
/_/\__,_/\__//____/\___/_/ /_/\____//____/
        xxxxxxxx xxx
                                       v",
        env!("CARGO_PKG_VERSION")
    )
}

pub fn new_test_thread(id: &str) -> ProcessId {
    let mut proc_data = ProcessData::new();
    proc_data.set_env("id", id);

    spawn_kernel_thread(func::test, format!("#{}_test", id), Some(proc_data))
}

pub fn new_stack_test_thread() {
    let pid = spawn_kernel_thread(func::stack_test, alloc::string::String::from("stack"), None);

    // wait for progress exit
    wait(pid);
}

/// use exit_code to determine whether the wait should end
fn wait(pid: ProcessId) {
    loop {
        let exit_code = manager::get_process_manager()
            .get_proc(&pid)
            .unwrap()
            .read()
            .exit_code();
        // if the process has exited, the exit_code exists
        if exit_code.is_none() {
            x86_64::instructions::hlt();
        } else {
            break;
        }
    }
}

const SHORT_UNITS: [&str; 4] = ["B", "K", "M", "G"];
const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];

pub fn humanized_size(size: u64) -> (f32, &'static str) {
    humanized_size_impl(size, false)
}

pub fn humanized_size_short(size: u64) -> (f32, &'static str) {
    humanized_size_impl(size, true)
}

#[inline]
pub fn humanized_size_impl(size: u64, short: bool) -> (f32, &'static str) {
    let bytes = size as f32;

    let units = if short { &SHORT_UNITS } else { &UNITS };

    let mut unit = 0;
    let mut bytes = bytes;

    while bytes >= 1024f32 && unit < units.len() {
        bytes /= 1024f32;
        unit += 1;
    }

    (bytes, units[unit])
}
