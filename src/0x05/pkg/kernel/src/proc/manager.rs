use super::*;
use crate::memory::get_frame_alloc_for_sure;
use alloc::{collections::*, format, sync::Arc, sync::Weak};
use spin::{Mutex, RwLock};
use vm::*;
use xmas_elf::ElfFile;

pub static PROCESS_MANAGER: spin::Once<ProcessManager> = spin::Once::new();

pub fn init(init: Arc<Process>, apps: boot::AppListRef) {
    // 1 set init process as Running
    init.write().resume();
    // info!("kproc has been setting as running");
    // info!("kproc status {:?}", init.read().status());

    // 2 set processor's current pid to init's pid
    processor::set_pid(init.pid());
    PROCESS_MANAGER.call_once(|| ProcessManager::new(init, apps));
}

pub fn get_process_manager() -> &'static ProcessManager {
    PROCESS_MANAGER
        .get()
        .expect("Process Manager has not been initialized")
}

pub struct ProcessManager {
    processes: RwLock<BTreeMap<ProcessId, Arc<Process>>>, // 用读写锁保护的进程键值对
    ready_queue: Mutex<VecDeque<ProcessId>>,              // 用于进程管理的双端队列
    app_list: boot::AppListRef,                           // 用户程序的列表
}

impl ProcessManager {
    pub fn new(init: Arc<Process>, apps: boot::AppListRef) -> Self {
        let mut processes = BTreeMap::new();
        let ready_queue = VecDeque::new();
        let pid = init.pid();

        trace!("Init {:#?}", init);

        processes.insert(pid, init);
        Self {
            processes: RwLock::new(processes),
            ready_queue: Mutex::new(ready_queue),
            app_list: apps,
        }
    }

    #[inline]
    pub fn app_list(&self) -> boot::AppListRef {
        self.app_list
    }

    #[inline]
    pub fn push_ready(&self, pid: ProcessId) {
        self.ready_queue.lock().push_back(pid);
    }

    #[inline]
    pub fn add_proc(&self, pid: ProcessId, proc: Arc<Process>) {
        self.processes.write().insert(pid, proc);
    }

    #[inline]
    pub fn get_proc(&self, pid: &ProcessId) -> Option<Arc<Process>> {
        self.processes.read().get(pid).cloned()
    }

    /// # Returns
    /// - Some(pid) => pid
    /// None => KERNEL_PID to meet phased requirement（阶段性需求）
    #[inline]
    pub fn pop_ready(&self) -> ProcessId {
        let id = match self.ready_queue.lock().pop_front() {
            Some(pid) => pid,
            _ => KERNEL_PID,
        };
        id
    }

    pub fn current(&self) -> Arc<Process> {
        self.get_proc(&processor::get_pid())
            .expect("No current process")
    }

    pub fn save_current(&self, context: &ProcessContext) {
        // 1. update current process's tick count
        let proc = self.current();
        proc.write().tick();

        // 2. save current process's context
        proc.write().save(context);
    }

    /// Blocking to obtain a ready process and switching to it
    pub fn switch_next(&self, context: &mut ProcessContext) -> ProcessId {
        loop {
            // 1. fetch the next process from ready queue

            let next_pid = self.pop_ready();
            let next_proc = match self.get_proc(&next_pid) {
                None => continue,
                Some(proc) => proc,
            };

            // 2. check if the next process is ready,
            // continue to fetch if not ready
            if next_proc.read().is_ready() {
                // 3. restore next process's context
                next_proc.write().restore(context);
                // 4. update processor's current pid
                processor::set_pid(next_pid);
                // 5. return next process's pid
                return next_pid;
            }
        }
    }

    pub fn kill_current(&self, ret: isize) -> bool {
        let pid = processor::get_pid().clone();
        if pid == KERNEL_PID {
            info!("The kernel process is under protected and can't be killed");
            return false;
        }
        self.kill(processor::get_pid(), ret);
        return true;
    }

    /// handle page fault
    ///
    /// # Returns
    /// - false
    ///  - if the fault caused by other unpredicted reason
    ///  - failed to handle the fault
    /// - true
    ///  - if the fault triggerd by PROTECTION_VIOLATION with CAUSED_BY_WRITE
    ///  - and handle it successfully
    pub fn handle_page_fault(&self, addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
        if !err_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION)
            && !err_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
        {
            return false;
        }
        self.current().write().handle_page_fault(addr)
    }

    pub fn kill(&self, pid: ProcessId, ret: isize) {
        let proc = self.get_proc(&pid);

        if proc.is_none() {
            warn!("Process #{} not found.", pid);
            return;
        }

        let proc = proc.unwrap();

        if proc.read().status() == ProgramStatus::Dead {
            warn!("Process #{} is already dead.", pid);
            return;
        }

        trace!("Kill {:#?}", &proc);
        info!("ret = {}", ret);
        proc.dealloc_current_stack();
        proc.kill(ret);
    }

    pub fn print_process_list(&self) {
        let mut output = String::from(format!(
            " {:>4} | {:>4} | {:12} | {:<7} | {:<7} | {:<12} | {:<7}\n",
            "PID", "PPID", "Process Name", "Ticks", "Status", "Memory Usage", "Percent"
        ));

        self.processes
            .read()
            .values()
            .filter(|p| p.read().status() != ProgramStatus::Dead)
            .for_each(|p| output += format!("{}\n", p).as_str());

        // Why get the mutex lock and drop it immediately?
        // - to eusure that the frame allocator exists but we don't require it this moment
        drop(get_frame_alloc_for_sure());

        output += format!("Queue  : {:?}\n", self.ready_queue.lock()).as_str();

        output += &processor::print_processors();

        print!("{}", output);
    }

    pub fn spawn(
        &self,
        elf: &ElfFile,
        name: String,
        parent: Option<Weak<Process>>,
        proc_data: Option<ProcessData>,
    ) -> ProcessId {
        let kproc = self.get_proc(&KERNEL_PID).unwrap();
        let page_table = kproc.read().clone_page_table();
        let proc_vm = Some(ProcessVm::new(page_table));
        let proc = Process::new(name, parent, proc_vm, proc_data);

        let mut inner = proc.write();
        // 1. use `load_elf` to process pagetable
        inner.load_elf(elf);
        drop(inner);

        // 2. alloc new stack for process
        let stack_top = proc.alloc_init_stack();
        let entry = VirtAddr::new(elf.header.pt2.entry_point());

        let mut inner = proc.write();
        inner.init_stack_frame(entry, stack_top);

        // 3. mark process as ready
        inner.pause();
        drop(inner);

        trace!("New {:#?}", &proc);
        let pid = proc.pid();

        // 4. something like kernel thread
        self.add_proc(pid, proc);
        self.push_ready(pid);
        pid
    }

    #[inline]
    pub fn write(&self, fd: u8, buf: &[u8]) -> isize {
        self.current().write().write(fd, buf)
    }

    #[inline]
    pub fn read(&self, fd: u8, buf: &mut [u8]) -> isize {
        self.current().read().read(fd, buf)
    }

    pub fn fork(&self) -> Arc<Process> {
        // FIXME: get current process
        let proc = self.current();
        // FIXME: fork to get child
        let child: Arc<Process> = proc.fork();
        // FIXME: add child to process list
        self.add_proc(child.pid(), child.clone());
        // FOR DBG: maybe print the process ready queue?
        self.print_process_list();

        return child;
    }
}
