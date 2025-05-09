use super::*;
use crate::memory::{
    self,
    allocator::{ALLOCATOR, HEAP_SIZE},
    get_frame_alloc_for_sure, PAGE_SIZE,
};
use alloc::{collections::*, format};
use spin::{Mutex, RwLock};

pub static PROCESS_MANAGER: spin::Once<ProcessManager> = spin::Once::new();

pub fn init(init: Arc<Process>) {
    // 传入了一个Arc，且引用process

    // FIXME: set init process as Running
    // 使用write()函数获取inner的写锁，然后resume()函数修改状态
    init.write().resume();

    // FIXME: set processor's current pid to init's pid
    // 调用processor.rs中的可用接口set_pid
    processor::set_pid(init.pid()); 
    PROCESS_MANAGER.call_once(|| ProcessManager::new(init));
}

pub fn get_process_manager() -> &'static ProcessManager {
    PROCESS_MANAGER
        .get()
        .expect("Process Manager has not been initialized")
}

pub struct ProcessManager {
    processes: RwLock<BTreeMap<ProcessId, Arc<Process>>>, // 用读写锁保护的进程键值对
    ready_queue: Mutex<VecDeque<ProcessId>>, // 用于进程管理的双端队列
}

impl ProcessManager {
    pub fn new(init: Arc<Process>) -> Self {
        let mut processes = BTreeMap::new();
        let ready_queue = VecDeque::new();
        let pid = init.pid();

        trace!("Init {:#?}", init);

        processes.insert(pid, init);
        Self {
            processes: RwLock::new(processes),
            ready_queue: Mutex::new(ready_queue),
        }
    }

    #[inline]
    pub fn push_ready(&self, pid: ProcessId) {
        self.ready_queue.lock().push_back(pid);
    } // .ready_queue.lock()返回互斥锁，.push_back()压入进程的pid

    #[inline]
    fn add_proc(&self, pid: ProcessId, proc: Arc<Process>) {
        self.processes.write().insert(pid, proc);
    }

    #[inline]
    fn get_proc(&self, pid: &ProcessId) -> Option<Arc<Process>> {
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
        let mut proc = self.current().write();
        // .current()返回Arc<process>，.write() 获取写锁保护着的ProcessInner
        // 谨慎Process内定义的方法.write()的返回类型！
        proc.tick(); // 调用ProcessInner的tick函数

        // FIXME: save current process's context
        proc.save(&context); // 调用ProcessInner的save函数
    }

    pub fn switch_next(&self, context: &mut ProcessContext) -> ProcessId {

        // FIXME: fetch the next process from ready queue
        let mut next_pid = self.pop_ready();
        let mut next_proc = self.get_proc(&next_pid);
        // FIXME: check if the next process is ready,
        //        continue to fetch if not ready
        loop {
            if next_proc.is_none() || next_proc.read().status() == ProgramStatus::Dead {
                next_pid = self.pop_ready();
                next_proc = self.get_proc(&next_pid);
                // 检测是否取出进程为空 or 取出进程状态是否为已被杀死
            } else {
                break;
            }
        }
        // FIXME: restore next process's context
        next_proc.write().restore(&context); // 调用ProcessInner中的restore()方法，将上下文写入context
        // FIXME: update processor's current pid
        processor::set_pid(&next_pid); // 调用Processor中的set_pid方法
        // FIXME: return next process's pid
        return next_pid;
    }

    pub fn spawn_kernel_thread(
        &self,
        entry: VirtAddr,
        name: String,
        proc_data: Option<ProcessData>,
    ) -> ProcessId {
        let kproc = self.get_proc(&KERNEL_PID).unwrap(); // 获取内核进程
        let page_table = kproc.read().clone_page_table(); // 获取内核进程的页表
        let proc_vm = Some(ProcessVm::new(page_table)); // 拷贝内科进程的页表
        let proc = Process::new(name, Some(Arc::downgrade(&kproc)), proc_vm, proc_data); // 复制内核进程，生成新的进程

        // alloc stack for the new process base on pid
        let stack_top = proc.alloc_init_stack();

        // FIXME: set the stack frame
        proc.write().init_stack_frame(entry, stack_top); // 调用ProcessContext中的方法 init_stack_frame

        // FIXME: add to process map
        let pid = proc.pid();
        self.add_proc(pid, proc); // 调用ProcessManager中的方法 add_proc
        // FIXME: push to ready queue
        self.push_ready(pid); // 调用ProcessManager中的方法 push_ready
        // FIXME: return new process pid
        pid
    }

    pub fn kill_current(&self, ret: isize) {
        self.kill(processor::get_pid(), ret);
    }

    pub fn handle_page_fault(&self, addr: VirtAddr, err_code: PageFaultErrorCode) -> bool {
        // FIXME: handle page fault
        if !err_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION) 
            && !err_code.contains(PageFaultErrorCode::CAUSED_BY_WRITE)
        {
            return false;
        } // 不是由于越权访问和写操作导致的 其他非预期错误 直接返回false
        self.current().write().handle_page_fault(addr) 
        // 调用ProcessInner中的相应缺页处理函数
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

        proc.kill(ret);
    }

    pub fn print_process_list(&self) {
        let mut output = String::from("  PID | PPID | Process Name |  Ticks  | Status\n");

        self.processes
            .read()
            .values()
            .filter(|p| p.read().status() != ProgramStatus::Dead)
            .for_each(|p| output += format!("{}\n", p).as_str());

        // TODO: print memory usage of kernel heap
        drop(get_frame_alloc_for_sure());
        // get_frame_alloc_for_sure()返回 帧分配器的互斥锁，确保 帧分配器存在
        // 因为不需要实际使用这个分配器，所以立刻通过drop()释放掉

        output += format!("Queue  : {:?}\n", self.ready_queue.lock()).as_str();

        output += &processor::print_processors();

        print!("{}", output);
    }
}
