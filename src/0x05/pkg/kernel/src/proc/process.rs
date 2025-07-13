use super::*;
use crate::memory::*;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::*;
use vm::*;
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::*;

use xmas_elf::ElfFile;

use crate::proc::vm::stack::{STACK_MAX_PAGES, STACK_START_MASK}; // 用于计算inner中的栈偏移量

#[derive(Clone)]
pub struct Process {
    pid: ProcessId,
    inner: Arc<RwLock<ProcessInner>>,
}

pub struct ProcessInner {
    name: String,
    parent: Option<Weak<Process>>,
    children: Vec<Arc<Process>>,
    ticks_passed: usize,
    status: ProgramStatus,
    context: ProcessContext,
    exit_code: Option<isize>,
    proc_data: Option<ProcessData>,
    proc_vm: Option<ProcessVm>,
}

impl Process {
    #[inline]
    pub fn pid(&self) -> ProcessId {
        self.pid
    }

    #[inline]
    pub fn write(&self) -> RwLockWriteGuard<ProcessInner> {
        self.inner.write()
    }

    #[inline]
    pub fn read(&self) -> RwLockReadGuard<ProcessInner> {
        self.inner.read()
    }

    pub fn new(
        name: String,
        parent: Option<Weak<Process>>,
        proc_vm: Option<ProcessVm>,
        proc_data: Option<ProcessData>,
    ) -> Arc<Self> {
        let name = name.to_ascii_lowercase();

        // create context
        let pid = ProcessId::new();
        let proc_vm = proc_vm.unwrap_or_else(|| ProcessVm::new(PageTableContext::new()));

        let inner = ProcessInner {
            name,
            parent,
            status: ProgramStatus::Ready,
            context: ProcessContext::default(),
            ticks_passed: 0,
            exit_code: None,
            children: Vec::new(),
            proc_vm: Some(proc_vm),
            proc_data: Some(proc_data.unwrap_or_default()),
        };

        trace!("New process {}#{} created.", &inner.name, pid);

        // create process struct
        Arc::new(Self {
            pid,
            inner: Arc::new(RwLock::new(inner)),
        })
    }

    pub fn kill(&self, ret: isize) {
        let mut inner = self.inner.write();

        debug!(
            "Killing process {}#{} with ret code: {}",
            inner.name(),
            self.pid,
            ret
        );

        inner.kill(ret);
    }

    pub fn alloc_init_stack(&self) -> VirtAddr {
        self.write().vm_mut().init_proc_stack(self.pid)
    }

    // 0x05 add:
    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        // FIXME: lock inner as write
        let mut inner = self.inner.write();
        // FIXME: inner fork with parent weak ref
        let child_inner = inner.fork(Arc::downgrade(self)); // 依然是逐级调用
        // FOR DBG: maybe print the child process info
        //          e.g. parent, name, pid, etc.
        let child_pid = ProcessId::new(); // 为子进程分配一个pid
        debug!(
            "Parent process: {} has forked {} with name {}",
            inner.name, child_pid, child_inner.name
        );
        // FIXME: make the arc of child
        let child_process = Arc::new(Self {
            pid: child_pid,
            inner: Arc::new(RwLock::new(child_inner)),
        }); // 仿照new方法的末尾的直接创建方法

        // FIXME: add child to current process's children list
        inner.children.push(child_process.clone()); // 注意这里同样要压入克隆体，不然会返回值出现借用错误
        // FIXME: set fork ret value for parent with `context.set_rax`
        inner.context.set_rax(child_pid.0 as usize);
        // FIXME: mark the child as ready & return it
        child_process.inner.write().pause(); // 注意这里不能再用child_inner了，那样写不进child_process……
        return child_process;
    }
}

impl ProcessInner {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn tick(&mut self) {
        self.ticks_passed += 1;
    }

    pub fn status(&self) -> ProgramStatus {
        self.status
    }

    pub fn pause(&mut self) {
        self.status = ProgramStatus::Ready;
    }

    pub fn resume(&mut self) {
        self.status = ProgramStatus::Running;
    }

    pub fn exit_code(&self) -> Option<isize> {
        self.exit_code
    }

    pub fn clone_page_table(&self) -> PageTableContext {
        self.proc_vm.as_ref().unwrap().page_table.clone_level_4()
    }

    pub fn is_ready(&self) -> bool {
        self.status == ProgramStatus::Ready
    }

    pub fn vm(&self) -> &ProcessVm {
        self.proc_vm.as_ref().unwrap()
    }

    pub fn vm_mut(&mut self) -> &mut ProcessVm {
        self.proc_vm.as_mut().unwrap()
    }

    pub fn handle_page_fault(&mut self, addr: VirtAddr) -> bool {
        self.vm_mut().handle_page_fault(addr)
    }

    /// Save the process's context
    /// mark the process as ready
    pub(super) fn save(&mut self, context: &ProcessContext) {
        // FIXME: save the process's context
        if self.is_dead() {
            return;
        }
        self.context.save(context); // 使用ProcessContext中定义的方法 save保存上下文
        self.pause(); // 调用方法pause()设置进程状态为 ready
    } // context中记录了原进程的上下文

    /// Restore the process's context
    /// mark the process as running
    pub(super) fn restore(&mut self, context: &mut ProcessContext) {
        // FIXME: restore the process's context
        self.context.restore(context); // 同样调用对应结构体方法 restore写入上下文

        // FIXME: restore the process's page table
        self.vm_mut().page_table.load(); // .vm_mut()得到ProcessVm, .load()调用对应PageTable方法加载上下文
        // ？这里需要使用vm_mut()吗？还是vm()就足以
        self.resume(); // 将进程的状态设置为 Running
    } // context是需要恢复的的上下文

    pub fn parent(&self) -> Option<Arc<Process>> {
        self.parent.as_ref().and_then(|p| p.upgrade())
    }

    pub fn kill(&mut self, ret: isize) {
        // FIXME: set exit code
        // 如果exit_code的值为None，表示进程尚未退出；为Some表示已经退出，并且获取到进程的返回值
        // 具体的进程的返回值是什么呢？
        self.exit_code = Some(ret);
        // FIXME: set status to dead
        self.status = ProgramStatus::Dead;

        // FIXME: take and drop unused resources
        // 使用Option提供的方法.take()，安全的取出并消费Option中的值
        self.proc_data.take();
        self.proc_vm.take();
    }

    pub fn init_stack_frame(&mut self, entry: VirtAddr, stack_top: VirtAddr) {
        self.context.init_stack_frame(entry, stack_top);
    }

    pub fn load_elf(&mut self, elf: &ElfFile) {
        self.vm_mut().load_elf(elf); // 调用ProcessVm中的load_elf()方法
    }

    pub fn is_dead(&self) -> bool {
        self.status == ProgramStatus::Dead
    }

    // 0x05
    pub fn fork(&mut self, parent: Weak<Process>) -> ProcessInner {
        // FIXME: fork the process virtual memory struct
        // FIXME: calculate the real stack offset
        let read_stack_offset = ((self.children.len() + 1) as u64) * STACK_MAX_PAGES;
        let child_vm = self.proc_vm.as_ref().unwrap().fork(read_stack_offset); // 保持逐级调用

        // FIXME: update `rsp` in interrupt stack frame
        let mut child_context: ProcessContext = self.context;
        let child_stack_top = (self.context.get_stack_top() & 0xFFFF_FFFF)
            | (child_vm.stack_start().as_u64() & STACK_START_MASK);
        child_context.update_rsp(child_stack_top);
        // FIXME: set the return value 0 for child with `context.set_rax`
        child_context.set_rax(0);
        // FIXME: clone the process data struct
        let child_data = self.proc_data.clone();
        // FIXME: construct the child process inner
        // NOTE: return inner because there's no pid record in inner
        Self {
            name: self.name.clone(),
            parent: Some(parent),
            children: Vec::new(),
            ticks_passed: 0,
            status: ProgramStatus::Ready, // rust要求必须初始化完整
            context: child_context,
            exit_code: None,
            proc_data: child_data,
            proc_vm: Some(child_vm),
        } // 仿照process中的new方法中新建一个inner结构体
    }

    pub fn block(&mut self) {
        self.status = ProgramStatus::Blocked;
    }

    pub fn set_rax(&mut self, ret: usize) {
        self.context.set_rax(ret);
    } // 添加一个方法便于manager.rs的wake_up中可以直接写

    // 0x05: 信号量相关的接口
    pub fn sem_init(&mut self, key: u32, value: usize) -> bool {
        self.proc_data.as_mut().unwrap().sem_init(key, value)
    }

    pub fn sem_remove(&mut self, key: u32) -> bool {
        self.proc_data.as_mut().unwrap().sem_remove(key)
    }

    pub fn sem_wait(&mut self, key: u32, pid: ProcessId) -> SemaphoreResult {
        self.proc_data.as_mut().unwrap().sem_wait(key, pid)
    }

    pub fn sem_signal(&mut self, key: u32) -> SemaphoreResult {
        self.proc_data.as_mut().unwrap().sem_signal(key)
    }
}

impl core::ops::Deref for Process {
    type Target = Arc<RwLock<ProcessInner>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl core::ops::Deref for ProcessInner {
    type Target = ProcessData;

    fn deref(&self) -> &Self::Target {
        self.proc_data
            .as_ref()
            .expect("Process data empty. The process may be killed.")
    }
}

impl core::ops::DerefMut for ProcessInner {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.proc_data
            .as_mut()
            .expect("Process data empty. The process may be killed.")
    }
}

impl core::fmt::Debug for Process {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let inner = self.inner.read();
        f.debug_struct("Process")
            .field("pid", &self.pid)
            .field("name", &inner.name)
            .field("parent", &inner.parent().map(|p| p.pid))
            .field("status", &inner.status)
            .field("ticks_passed", &inner.ticks_passed)
            .field("children", &inner.children.iter().map(|c| c.pid.0))
            .field("status", &inner.status)
            .field("context", &inner.context)
            .field("vm", &inner.proc_vm)
            .finish()
    }
}

impl core::fmt::Display for Process {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let inner = self.inner.read();
        write!(
            f,
            " #{:-3} | #{:-3} | {:12} | {:7} | {:?}",
            self.pid.0,
            inner.parent().map(|p| p.pid.0).unwrap_or(0),
            inner.name,
            inner.ticks_passed,
            inner.status
        )?;
        Ok(())
    }
}
