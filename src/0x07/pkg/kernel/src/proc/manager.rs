use super::*;
use crate::memory::{
    self, PAGE_SIZE,
    allocator::{ALLOCATOR, HEAP_SIZE},
    get_frame_alloc_for_sure,
};
use crate::utils::humanized_size;
use alloc::{collections::*, format, sync::Arc, sync::Weak};
use core::ops::DerefMut;
use spin::{Mutex, RwLock};
use vm::*;

use xmas_elf::ElfFile;

pub static PROCESS_MANAGER: spin::Once<ProcessManager> = spin::Once::new();

pub fn init(init: Arc<Process>, apps: boot::AppListRef) {
    // 传入了一个Arc，且引用process

    // FIXME: set init process as Running
    // 使用write()函数获取inner的写锁，然后resume()函数修改状态
    init.write().resume();
    // info!("kproc has been setting as running");
    // info!("kproc status {:?}", init.read().status());

    // FIXME: set processor's current pid to init's pid
    // 调用processor.rs中的可用接口set_pid
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
    app_list: boot::AppListRef, // 0x04: 采用boot/lib.rs中定义的Option<&AppList>
    wait_queue: Mutex<BTreeMap<ProcessId, BTreeSet<ProcessId>>>, // 0x05: 等待队列
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
            wait_queue: Mutex::new(BTreeMap::new()), // 0x05 add
        }
    }

    #[inline]
    pub fn app_list(&self) -> boot::AppListRef {
        self.app_list
    }

    #[inline]
    pub fn push_ready(&self, pid: ProcessId) {
        self.ready_queue.lock().push_back(pid);
    } // .ready_queue.lock()返回互斥锁，.push_back()压入进程的pid

    #[inline]
    pub fn add_proc(&self, pid: ProcessId, proc: Arc<Process>) {
        self.processes.write().insert(pid, proc);
    }

    #[inline]
    pub fn get_proc(&self, pid: &ProcessId) -> Option<Arc<Process>> {
        self.processes.read().get(pid).cloned()
    }

    #[inline]
    pub fn pop_ready(&self) -> ProcessId {
        let id = match self.ready_queue.lock().pop_front() {
            Some(pid) => pid,
            _ => KERNEL_PID,
            // 当没有对应进程的时候，返回内核进程的pid，满足阶段性成果需求
        };
        id
    } // 仿照上述的push_ready()函数，取出双端队列的队头进程的pid

    pub fn current(&self) -> Arc<Process> {
        self.get_proc(&processor::get_pid())
            .expect("No current process")
    }

    pub fn save_current(&self, context: &ProcessContext) {
        // FIXME: update current process's tick count
        let proc = self.current();
        // .current()返回Arc<process>
        // 谨慎Process内定义的方法.write()的返回类型！
        // 需要.write() 获取写锁保护着的ProcessInner
        proc.write().tick(); // 调用ProcessInner的tick函数

        // FIXME: save current process's context
        proc.write().save(context); // 调用ProcessInner的save函数
    }

    pub fn switch_next(&self, context: &mut ProcessContext) -> ProcessId {
        loop {
            // FIXME: fetch the next process from ready queue

            // FIXME: check if the next process is ready,
            //        continue to fetch if not ready
            let next_pid = self.pop_ready();
            let next_proc = match self.get_proc(&next_pid) {
                None => continue,
                Some(proc) => proc,
            };

            if next_proc.read().is_ready() {
                // FIXME: restore next process's context
                next_proc.write().restore(context); // 调用ProcessInner中的restore()方法，将上下文写入context
                // FIXME: update processor's current pid
                processor::set_pid(next_pid); // 调用Processor中的set_pid方法
                // FIXME: return next process's pid
                return next_pid;
            }
        }
    }

    pub fn kill_current(&self, ret: isize) {
        self.kill(processor::get_pid(), ret);
    }

    pub fn handle_page_fault(&self, addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
        // FIXME: handle page fault
        if !err_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE) {
            return false;
        } // 不是由于越权访问和写操作导致的 其他非预期错误 直接返回false
        self.current().write().handle_page_fault(addr)
        // 调用ProcessInner中的相应缺页处理函数
    } // 用于处理缺页异常的函数，在无法解决的情况下返回false，
    // 可能解决的情况：调用ProcessInner中的 handle_page_fault 函数

    pub fn kill(&self, pid: ProcessId, ret: isize) {
        let proc = self.get_proc(&pid);

        if proc.is_none() {
            warn!("Process #{} not found.", pid);
            return;
        }
        // 0x05 add
        if let Some(pids) = self.wait_queue.lock().remove(&pid) {
            for pid in pids {
                self.wake_up(pid, Some(ret));
            }
        } // finish add

        let proc = proc.unwrap();

        if proc.read().status() == ProgramStatus::Dead {
            warn!("Process #{} is already dead.", pid);
            return;
        }

        trace!("Kill {:#?}", &proc);
        info!("ret = {}", ret);
        proc.kill(ret);
    }

    pub fn print_process_list(&self) {
        let mut output =
            String::from("  PID | PPID | Process Name |  Ticks  |   Memory  | Status\n");
        // 要注意这里要添加一项，验收的时候忘记了0x07会显示对应进程的内存
        self.processes
            .read()
            .values()
            .filter(|p| p.read().status() != ProgramStatus::Dead)
            .for_each(|p| output += format!("{}\n", p).as_str());

        // TODO: print memory usage of kernel heap
        drop(get_frame_alloc_for_sure());
        // get_frame_alloc_for_sure()返回 帧分配器的互斥锁，确保 帧分配器存在
        // 因为不需要实际使用这个分配器，所以立刻通过drop()释放掉

        // NOTE: print memory page usage
        //      (you may implement following functions)
        let alloc = get_frame_alloc_for_sure();
        let frames_used = alloc.frames_used();
        let frames_recycled = alloc.frames_recycled();
        let frames_total = alloc.frames_total();

        let used = (frames_used - frames_recycled) * PAGE_SIZE as usize;
        let total = frames_total * PAGE_SIZE as usize;

        output += &Self::format_usage("Memory", used, total);
        drop(alloc);

        output += format!("Queue  : {:?}\n", self.ready_queue.lock()).as_str();

        output += &processor::print_processors();

        print!("{}", output);
    }

    // 0x04 add
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
        // FIXME: load elf to process pagetable
        inner.load_elf(elf); // 调用ProcessInner中的load_elf()
        drop(inner);
        // FIXME: alloc new stack for process
        let stack_top = proc.alloc_init_stack();
        let entry = VirtAddr::new(elf.header.pt2.entry_point());

        let mut inner = proc.write();
        inner.init_stack_frame(entry, stack_top);
        // FIXME: mark process as ready
        inner.pause();
        drop(inner);

        trace!("New {:#?}", &proc);

        let pid = proc.pid();
        // FIXME: something like kernel thread
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

    // 0x05 add:
    // 选择了同样返回一个子进程的Arc引用，因为Manager中需要获取它的pid
    pub fn fork(&self) -> Arc<Process> {
        // FIXME: get current process
        let current_process = self.current();
        // FIXME: fork to get child
        let child: Arc<Process> = current_process.fork(); // 逐层调用
        // FIXME: add child to process list
        self.add_proc(child.pid(), child.clone()); // 这里压入克隆体，防止借用
        // FOR DBG: maybe print the process ready queue?
        debug!("The process ready queue: {:?}", self.ready_queue.lock());
        child
    }

    /// Block the process with the given pid
    pub fn block(&self, pid: &ProcessId) {
        if let Some(proc) = self.get_proc(&pid) {
            // FIXME: set the process as blocked
            proc.write().block(); // 调用ProcessInner中的对应函数
        }
    }

    pub fn wait_pid(&self, pid: ProcessId) {
        let mut wait_queue = self.wait_queue.lock();
        // FIXME: push the current process to the wait queue
        //        `processor::get_pid()` is waiting for `pid`
        let entry = wait_queue.entry(pid).or_default();
        entry.insert(processor::get_pid());
    }

    pub fn get_exit_code(&self, pid: ProcessId) -> Option<isize> {
        let exit_code = match self.processes.read().get(&pid) {
            Some(proc) => {
                if proc.read().is_dead() {
                    proc.read().exit_code()
                } else {
                    None
                }
            }
            _ => None,
        };
        exit_code
    }

    /// Wake up the process with the given pid
    ///
    /// If `ret` is `Some`, set the return value of the process
    pub fn wake_up(&self, pid: ProcessId, ret: Option<isize>) {
        if let Some(proc) = self.get_proc(&pid) {
            let mut inner = proc.write();
            if let Some(ret) = ret {
                // FIXME: set the return value of the process
                //        like `context.set_rax(ret as usize)`
                inner.set_rax(ret as usize);
            }
            // FIXME: set the process as ready
            inner.pause();
            // FIXME: push to ready queue
            self.push_ready(pid);
        }
    }

    // 0x07 add
    // A helper function to format memory usage
    pub fn format_usage(name: &str, used: usize, total: usize) -> String {
        let (used_float, used_unit) = humanized_size(used as u64);
        let (total_float, total_unit) = humanized_size(total as u64);

        format!(
            "{:<6} : {:>6.*} {:>3} / {:>6.*} {:>3} ({:>5.2}%)\n",
            name,
            2,
            used_float,
            used_unit,
            2,
            total_float,
            total_unit,
            used as f32 / total as f32 * 100.0
        )
    }
}
