use super::consts::*;
// 这里引入原子类型不能时用std::sync::atomic 因为 no_std环境!
use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::memory::gdt; // 在设置中断栈的时候需要使用对应的中断栈栈号
use crate::proc::ProcessContext;
use crate::utils::regs::*; // 在as_handler需要进程上下文

pub unsafe fn register_idt(idt: &mut InterruptDescriptorTable) {
    // 以偏移量的方式设置中断号，这里的实际中断向量号为32
    // 在003中，要求将时钟中断栈加载到idt中
    unsafe {
        idt[Interrupts::IrqBase as u8 + Irq::Timer as u8]
            .set_handler_fn(clock_handler)
            .set_stack_index(gdt::CLOCK_IST_INDEX);
    }
}

// 0x03 add：利用as_handler宏重新定义中断处理函数
pub extern "C" fn clock(mut context: ProcessContext) {
    crate::proc::switch(&mut context);
    super::ack(); // 用于通知中断控制器中断处理已完成
}

as_handler!(clock);

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
