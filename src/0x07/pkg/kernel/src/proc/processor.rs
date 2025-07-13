use core::sync::atomic::{AtomicU16, Ordering};

use crate::proc::ProcessId;
use alloc::{string::String, vec::Vec};
use x86::cpuid::CpuId;

const MAX_CPU_COUNT: usize = 4;
// ?为什么这里的 CPU_COUNT 定义为4？

#[allow(clippy::declare_interior_mutable_const)]
const EMPTY: Processor = Processor::new(); // means no process

static PROCESSORS: [Processor; MAX_CPU_COUNT] = [EMPTY; MAX_CPU_COUNT];

/// Returns the current processor based on the current APIC ID
fn current() -> &'static Processor {
    let cpuid = CpuId::new()
        .get_feature_info()
        .unwrap()
        .initial_local_apic_id() as usize;

    &PROCESSORS[cpuid]
} // 返回当前正在使用的处理器

pub fn print_processors() -> String {
    alloc::format!(
        "CPUs   : {}\n",
        PROCESSORS
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.is_free())
            .map(|(i, p)| alloc::format!("[{}: {}]", i, p.get_pid().unwrap()))
            .collect::<Vec<_>>()
            .join(", ")
    )
} // 打印不同处理器的状态

/// Processor holds the current process id
pub struct Processor(AtomicU16);

impl Processor {
    pub const fn new() -> Self {
        Self(AtomicU16::new(0))
    }
}

#[inline]
pub fn set_pid(pid: ProcessId) {
    current().set_pid(pid)
}

#[inline]
pub fn get_pid() -> ProcessId {
    current().get_pid().expect("No current process")
}

impl Processor {
    #[inline]
    pub fn is_free(&self) -> bool {
        // 初始设置下pid为0 证明处理器为空
        self.0.load(Ordering::Relaxed) == 0
    } // 判断处理器是否为空

    #[inline]
    pub fn set_pid(&self, pid: ProcessId) {
        self.0.store(pid.0, Ordering::Relaxed);
    } // 将当前进程的pid存入处理器

    #[inline]
    pub fn get_pid(&self) -> Option<ProcessId> {
        let pid = self.0.load(Ordering::Relaxed);
        if pid == 0 { None } else { Some(ProcessId(pid)) }
    } // 获取处理器当前进程的pid，如果处理器为空则返回None
}
