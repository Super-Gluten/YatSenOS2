use super::consts::*;
// 这里引入原子类型不能时用std::sync::atomic 因为 no_std环境!
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    // 以偏移量的方式设置中断号，这里的实际中断向量号为32
    idt[Interrupts::IrqBase as u8 + Irq::Timer as u8].set_handler_fn(clock_handler);
}

pub extern "x86-interrupt" fn clock_handler(_sf: InterruptStackFrame) {
    // 在禁用中断的上下文中执行，防止嵌套中断导致竞态条件
    x86_64::instructions::interrupts::without_interrupts(|| {
        if inc_counter() % 0x100000 == 0 {
            // if inc_counter() % 0x10000 == 0 {
            info!("Tick! @{}", read_counter());
        }
        // 用于通知中断控制器中断处理已完成
        super::ack();
    });
}

// static COUNTER: /* FIXME */ = /* FIXME */;
// 使用全局原子计数器，保证原子性
static COUNTER: AtomicU64 = AtomicU64::new(0);

#[inline]
// #[inline]提示编译器内联优化
pub fn read_counter() -> u64 {
    // FIXME: load counter value
    // 安全读取计数器的当前值
    // Relaxed排序不依赖其他内存操作，只需保证原子性
    // 而SeqCst有严格顺序一致性，如果怀疑原子操作问题，可以临时替换
    COUNTER.load(Ordering::SeqCst)
}

#[inline]
pub fn inc_counter() -> u64 {
    // FIXME: read counter value and increase it
    // 原子化先读取后 +1
    COUNTER.fetch_add(1, Ordering::SeqCst)
}
