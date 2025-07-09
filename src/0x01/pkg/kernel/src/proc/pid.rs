use core::sync::atomic::{AtomicU16, Ordering};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessId(pub u16);

static COUNTER : AtomicU16 = AtomicU16::new(1);
// 调用一个静态的原子变量作为PID的计数
// 初始定义为1是因为在mod.rs中，常量定义了KERNEL_PID 为1，保持一致

impl ProcessId {
    pub fn new() -> Self {
        // FIXME: Get a unique PID
        let pid = COUNTER.fetch_add(1, Ordering::SeqCst);
        // 每次生成新的进程，静态变量COUNTER永久加1
        Self(pid)
    }
}

impl Default for ProcessId {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Display for ProcessId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::fmt::Debug for ProcessId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<ProcessId> for u16 {
    fn from(pid: ProcessId) -> Self {
        pid.0
    }
}
