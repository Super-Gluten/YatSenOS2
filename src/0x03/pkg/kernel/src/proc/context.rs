use volatile::{VolatileRef, access::ReadOnly};
use x86_64::structures::gdt::SegmentSelector;
use x86_64::{VirtAddr, registers::rflags::RFlags, structures::idt::InterruptStackFrameValue}; // 在Default内的SegmentSelector需要使用

use crate::{RegistersValue, memory::gdt::get_selector};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ProcessContextValue {
    pub regs: RegistersValue,                  // 使用utils/reg.rs中定义的结构体
    pub stack_frame: InterruptStackFrameValue, // 使用x86_64库中idt的中断栈值
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ProcessContext {
    value: ProcessContextValue,
}

impl ProcessContext {
    #[inline]
    pub fn as_mut(&mut self) -> VolatileRef<ProcessContextValue> {
        VolatileRef::from_mut_ref(&mut self.value)
    } // 返回一个对 ProcessContextValue的易变引用包装器，修改易变数据时使用
    // VolatileRef：特殊的引用包装器，用于处理 易变内存访问，防止编译器进行不安全的优化

    #[inline]
    pub fn as_ref(&self) -> VolatileRef<'_, ProcessContextValue, ReadOnly> {
        VolatileRef::from_ref(&self.value)
    } // 返回一个只读的VolatileRef包装器，安全读取易变数据，只读时使用
    // as_mut 和 as_ref 用于获取可变/只读的 VolatileRef包装器
    // 对应使用 .as_mut_ptr() 和 .as_ptr() 获取底层指针，执行不可优化的内存访问操作

    #[inline]
    pub fn set_rax(&mut self, value: usize) {
        self.value.regs.rax = value;
    }

    #[inline]
    pub fn save(&mut self, context: &ProcessContext) {
        self.value = context.as_ref().as_ptr().read();
    } // 将context保存到当前对象

    #[inline]
    pub fn restore(&self, context: &mut ProcessContext) {
        context.as_mut().as_mut_ptr().write(self.value);
    } // 将当前对象的上下文写入context

    pub fn init_stack_frame(&mut self, entry: VirtAddr, stack_top: VirtAddr) {
        self.value.stack_frame.stack_pointer = stack_top;
        self.value.stack_frame.instruction_pointer = entry;
        self.value.stack_frame.cpu_flags =
            RFlags::IOPL_HIGH | RFlags::IOPL_LOW | RFlags::INTERRUPT_FLAG;

        let selector = get_selector();
        self.value.stack_frame.code_segment = selector.code_selector;
        self.value.stack_frame.stack_segment = selector.data_selector;

        trace!("Init stack frame: {:#?}", &self.stack_frame);
    }
}

impl Default for ProcessContextValue {
    fn default() -> Self {
        Self {
            regs: RegistersValue::default(),
            stack_frame: InterruptStackFrameValue::new(
                VirtAddr::new(0x1000),
                SegmentSelector(0),
                RFlags::empty(),
                VirtAddr::new(0x2000),
                SegmentSelector(0),
            ),
        }
    }
}

impl core::ops::Deref for ProcessContext {
    type Target = ProcessContextValue;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl core::fmt::Debug for ProcessContext {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        self.value.fmt(f)
    }
}

impl core::fmt::Debug for ProcessContextValue {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut f = f.debug_struct("StackFrame");
        f.field("stack_top", &self.stack_frame.stack_pointer);
        f.field("cpu_flags", &self.stack_frame.cpu_flags);
        f.field("instruction_pointer", &self.stack_frame.instruction_pointer);
        f.field("regs", &self.regs);
        f.finish()
    }
}
