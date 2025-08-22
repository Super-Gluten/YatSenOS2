use super::*;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::*;
use vm::*;

use crate::humanized_size;
use crate::proc::vm::stack::{STACK_MAX_PAGES, STACK_START_MASK};
use stack::STACK_MAX_SIZE;
use xmas_elf::ElfFile;

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

    pub fn dealloc_current_stack(&self) {
        self.write().vm_mut().clean_up_stack()
    }

    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        // 1. lock inner as write
        let mut inner = self.write();
        
        // 2. inner fork with parent weak ref
        let child_inner = inner.fork(Arc::downgrade(self));
        let child_pid = ProcessId::new();
        // FOR DBG: maybe print the child process info
        //          e.g. parent, name, pid, etc.
        trace!(
            "the process {} fork a child with name: {}, with pid {}", 
            self.pid.0, child_inner.name(), child_pid.0,
        );
        
        // 3. make the arc of child
        let child = Arc::new(
            Self {
                pid: child_pid,
                inner: Arc::new(RwLock::new(child_inner)),
            });
        
        // 4. add child to current process's children list
        inner.add_child(child.clone());

        // 5.set fork ret value for parent with `context.set_rax`
        inner.context.set_rax(child_pid.0 as usize);

        // 6. mark the child as ready & return it
        child.write().pause();
        return child;
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

    pub fn block(&mut self) {
        self.status = ProgramStatus::Blocked;
    }

    /// # Returns
    /// - none if process still alive.
    /// - Some(ret) if process is dead
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
        // 1. save the process's context if the process is alive
        if self.status == ProgramStatus::Dead {
            return;
        }
        self.context.save(context);

        // 2. mark the process as ready
        self.pause();
    }

    /// Restore the process's context
    /// mark the process as running
    pub(super) fn restore(&mut self, context: &mut ProcessContext) {
        // 1. restore the process's context
        self.context.restore(context);

        // 2. restore the process's page table
        self.vm_mut().page_table.load();

        // 3. mark the process as running
        self.resume();
    }

    pub fn parent(&self) -> Option<Arc<Process>> {
        self.parent.as_ref().and_then(|p| p.upgrade())
    }

    pub fn init_stack_frame(&mut self, entry: VirtAddr, stack_top: VirtAddr) {
        self.context.init_stack_frame(entry, stack_top);
    }

    fn kill(&mut self, ret: isize) {
        // 1. set exit code
        self.exit_code = Some(ret);
        // 2. set status to dead
        self.status = ProgramStatus::Dead;

        // 3. take and drop unused resources
        self.proc_data.take();
        self.proc_vm.take();
    }

    pub fn load_elf(&mut self, elf: &ElfFile) {
        self.vm_mut().load_elf(elf); // 调用ProcessVm中的load_elf()方法
    }

    pub fn is_dead(&self) -> bool {
        self.status == ProgramStatus::Dead
    }

    pub fn fork(&mut self, parent: Weak<Process>) -> ProcessInner {
        // 1. calculate the real stack offset
        //
        // - `real_stack_offset_count` determined by the number of child processes
        let real_stack_offset_count: u64 = STACK_MAX_PAGES * (self.children.len() + 1) as u64;
        
        // 2. fork the process virtual memory struct
        let child_vm = self.vm_mut().fork(real_stack_offset_count);
        
        // 3. update `rsp` in interrupt stack frame
        //
        //  - `current_stack_top_in_low` 代表栈顶于进程栈空间的相对位置
        //  - `child_stack_top_in_high` 是子进程的栈空间基址
        let mut child_context = self.context;
        let current_stack_top_in_low = self.context.get_rsp().as_u64() & (STACK_MAX_SIZE - 1);
        let child_stack_top_in_high = child_vm.stack.stack_start().as_u64() & STACK_START_MASK;
        
        let child_stack_top = current_stack_top_in_low | child_stack_top_in_high;
        child_context.update_rsp(child_stack_top);
        trace!("parent's rsp is {:#x}, child's rsp is {:#x}", self.context.get_rsp(), child_context.get_rsp());
        
        // 4. set the return value 0 for child with `context.set_rax`
        child_context.set_rax(0);

        // 5. clone the process data struct
        let child_data = self.proc_data.clone().unwrap();
        
        // 6. construct the child process inner
        ProcessInner {
            name: self.name.clone(),
            parent: Some(parent),
            children: Vec::new(),
            ticks_passed: 0,
            status: ProgramStatus::Ready,
            context: child_context,
            exit_code: None,
            proc_data: Some(child_data),
            proc_vm: Some(child_vm),
        }
        // NOTE: return inner because there's no pid record in inner
    }

    pub fn add_child(&mut self, child: Arc<Process>) {
        self.children.push(child);
    }

    pub fn set_rax(&mut self, value: usize) {
        self.context.set_rax(value);
    }

    pub fn new_sem(&mut self, key: u32, value: usize) -> usize {
        self.proc_data.as_mut().unwrap().new_sem(key, value)
    }

    pub fn remove_sem(&mut self, key: u32) -> usize {
        self.proc_data.as_mut().unwrap().remove_sem(key)
    }

    pub fn sem_signal(&mut self, key: u32) -> SemaphoreResult {
        self.proc_data.as_mut().unwrap().sem_signal(key)
    }

    pub fn sem_wait(&mut self, key: u32, pid: ProcessId) -> SemaphoreResult {
        self.proc_data.as_mut().unwrap().sem_wait(key, pid)
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
        let memory_size = inner.vm().memory_usage();
        let (size, unit) = humanized_size(memory_size);
        let stack_usage_percent = (memory_size as f64) / (STACK_MAX_SIZE as f64) * 100.0;

        write!(
            f,
            " #{:-3} | #{:-3} | {:12} | {:<7} | {:<7} | {:<12} | {:.2}%",
            self.pid.0,
            inner.parent().map(|p| p.pid.0).unwrap_or(0),
            inner.name,
            inner.ticks_passed,
            format!("{}", inner.status),
            format!("{}{}", size, unit),
            stack_usage_percent,
        )?;
        Ok(())
    }
}
