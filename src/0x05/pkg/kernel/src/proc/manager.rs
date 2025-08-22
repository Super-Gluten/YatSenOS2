//! ### `BTreeMap` 相关操作
//! | 函数/方法                         | 作用                                                                 |
//! |--------------------------------- |----------------------------------------------------------------------|
//! | `BTreeMap::new()`                | 创建一个新的空 `BTreeMap`                                             |
//! | `map.insert(key, value)`         | 插入键值对，返回该键之前对应的值（若存在则为 `Some(old_value)`，否则为 `None`） |
//! | `map.get(&key)`                  | 获取键对应的值的不可变引用，返回 `Option<&V>`                          |
//! | `map.get_mut(&key)`              | 获取键对应的值的可变引用，返回 `Option<&mut V>`                        |
//! | `map.contains_key(&key)`         | 检查映射中是否包含指定键，返回布尔值                                   |
//! | `map.remove(&key)`               | 移除指定键对应的键值对，返回被移除的值（若存在则为 `Some(value)`）      |
//! | `map.entry(key).or_insert(value)`| 若键不存在则插入默认值，返回该键对应值的可变引用                         |
//! | `map.range(a..b)`                | 返回键在 `[a, b)` 范围内的键值对迭代器                                 |
//! | `map.keys()`                     | 返回所有键的迭代器（按排序顺序）                                       |
//! | `map.values()`                   | 返回所有值的迭代器（按键的排序顺序）                                   |
//!
//! ### `BTreeSet` 相关操作
//! | 函数/方法                         | 作用                                                                 |
//! |--------------------------------- |----------------------------------------------------------------------|
//! | `BTreeSet::new()`                | 创建一个新的空 `BTreeSet`                                             |
//! | `set.insert(value)`              | 插入元素，若元素已存在则返回 `false`，否则返回 `true`                  |
//! | `set.contains(&value)`           | 检查集合中是否包含指定元素，返回布尔值                                 |
//! | `set.remove(&value)`             | 移除指定元素，返回是否成功移除（存在则为 `true`）                       |
//! | `set.range(a..b)`                | 返回元素在 `[a, b)` 范围内的迭代器（按排序顺序）                       |
//! | `set.union(&other)`              | 返回与另一个集合的并集迭代器（包含两集合中所有不重复元素）              |
//! | `set.intersection(&other)`       | 返回与另一个集合的交集迭代器（包含两集合共有的元素）                    |
//! | `set.difference(&other)`         | 返回与另一个集合的差集迭代器（包含本集合有而另一个集合没有的元素）      |
//! | `set.is_subset(&other)`          | 检查本集合是否为另一个集合的子集（所有元素都在另一个集合中）            |
//!
//! ## 示例代码
//! ```rust
//! use std::collections::{BTreeMap, BTreeSet};
//!
//! // 1. BTreeMap 操作
//! let mut map = BTreeMap::new();
//! map.insert(2, "two");
//! map.insert(1, "one");
//! assert_eq!(map.get(&1), Some(&"one"));
//!
//! // 使用 entry API 插入或修改
//! map.entry(3).or_insert("three");
//! assert!(map.contains_key(&3));
//!
//! // 2. BTreeSet 操作
//! let mut set = BTreeSet::new();
//! set.insert(3);
//! set.insert(1);
//! set.insert(2);
//! assert!(set.contains(&2));
//!
//! // 集合运算
//! let other = BTreeSet::from([2, 3, 4]);
//! let intersection: BTreeSet<_> = set.intersection(&other).cloned().collect();
//! assert_eq!(intersection, BTreeSet::from([2, 3]));
//! ```
//!
//! ## 注意事项（Attention）
//! 1. **排序要求**：键（`BTreeMap`）和元素（`BTreeSet`）必须实现 `Ord` trait 以保证排序性。
//! 2. **唯一性**：`BTreeMap` 的键和 `BTreeSet` 的元素都是唯一的，重复插入会被覆盖或忽略。
//! 3. **性能特性**：插入、删除、查询操作的平均时间复杂度为 O(log n)，适合需要有序访问的场景。
//!
//! 更多细节参考官方文档：
//! - [`std::collections::BTreeMap`](https://doc.rust-lang.org/std/collections/struct.BTreeMap.html)
//! - [`std::collections::BTreeSet`](https://doc.rust-lang.org/std/collections/struct.BTreeSet.html)

use super::*;
use crate::memory::get_frame_alloc_for_sure;
use alloc::{collections::*, vec::Vec, format, sync::Arc, sync::Weak};
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
    processes: RwLock<BTreeMap<ProcessId, Arc<Process>>>,           // 用读写锁保护的进程键值对
    ready_queue: Mutex<VecDeque<ProcessId>>,                        // 用于进程管理的双端队列
    app_list: boot::AppListRef,                                     // 用户程序的列表
    wait_queue: Mutex<BTreeMap<ProcessId, BTreeSet<ProcessId>>>,    // 用于存储等待的进程ID的集合
    block_queue: Mutex<BTreeMap<ProcessId, BTreeSet<ProcessId>>>,   // 用于查询阻塞当前进程的进程ID集合
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
            wait_queue: Mutex::new(BTreeMap::new()),
            block_queue: Mutex::new(BTreeMap::new()),
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
    /// None => KERNEL_PID to meet phased requirement
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

    pub fn kill(&self, proc_pid: ProcessId, ret: isize) {
        let proc = self.get_proc(&proc_pid);

        if proc.is_none() {
            warn!("Process #{} not found.", proc_pid);
            return;
        }

        let proc = proc.unwrap();

        if proc.read().status() == ProgramStatus::Dead {
            warn!("Process #{} is already dead.", proc_pid);
            return;
        }

        trace!("Kill {:#?}", &proc);
        debug!("ret = {}", ret);
        proc.dealloc_current_stack();
        proc.kill(ret);

        // remove correspond value set and wake up those process
        if let Some(pids) = self.wait_queue.lock().remove(&proc_pid) {
            for pid in pids {
                let mut current_map = self.block_queue.lock();
                let current_set = current_map.get_mut(&pid).unwrap();
                current_set.remove(&proc_pid);
                if current_set.is_empty() {
                    self.wake_up(pid, Some(ret));
                } else {

                }
            }
        }
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
        // 1. get current process
        let proc = self.current();
        // 2. fork to get child
        let child: Arc<Process> = proc.fork();
        // 3. add child to process list
        self.add_proc(child.pid(), child.clone());
        // FOR DBG: maybe print the process ready queue?
        // self.print_process_list();

        return child;
    }

    /// Block the process with the given pid
    pub fn block(&self, pid: ProcessId) {
        if let Some(proc) = self.get_proc(&pid) {
            proc.write().block();
        }
    }

    /// Add to `wait_queue` with given pid
    pub fn wait_pid(&self, blocking_pid: ProcessId) {
        let blocked_pid = processor::get_pid();
        let mut wait_queue = self.wait_queue.lock();
        // choose `pid` as key and `processor::get_pid()` as value
        // to insert into `wait_queue`
        wait_queue
            .entry(blocking_pid)
            .or_default()
            .insert(blocked_pid);

        let mut block_queue = self.block_queue.lock();
        block_queue
            .entry(blocked_pid)
            .or_default()
            .insert(blocking_pid);
    }

    /// Wake up the process with the given pid
    ///
    /// If `ret` is `Some`, set the return value of the process
    pub fn wake_up(&self, pid: ProcessId, ret: Option<isize>) {
        if let Some(proc) = self.get_proc(&pid) {
            let mut inner = proc.write();
            if let Some(ret) = ret {
                // set the return value of the process
                // like `context.set_rax(ret as usize)`
                inner.set_rax(ret as usize);
            }
            // set the process as ready
            // push to ready queue
            inner.pause();
            self.push_ready(pid);
        }
    }

    pub fn query_block(&self, query_pid: ProcessId) -> Vec<ProcessId> {
        let block_queue = self.block_queue.lock();
        let mut ret_vec: Vec<ProcessId> = Vec::new();
        match block_queue.get(&query_pid) {
            Some(set) => {
                for item in set.iter() {
                    ret_vec.push(*item);
                }
            }
            None => {}
        };
        ret_vec
    }
}
