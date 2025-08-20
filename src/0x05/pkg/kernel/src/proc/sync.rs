use super::ProcessId;
use alloc::collections::*;
use spin::Mutex;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SemaphoreId(u32);

impl SemaphoreId {
    pub fn new(key: u32) -> Self {
        Self(key)
    }
}

/// Mutex is required for Semaphore
#[derive(Debug, Clone)]
pub struct Semaphore {
    count: usize,
    wait_queue: VecDeque<ProcessId>,
}

/// Semaphore result
#[derive(Debug)]
pub enum SemaphoreResult {
    Ok,
    NotExist,
    Block(ProcessId),
    WakeUp(ProcessId),
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(value: usize) -> Self {
        Self {
            count: value,
            wait_queue: VecDeque::new(),
        }
    }

    /// Wait the semaphore (acquire/down/proberen)
    ///
    /// # Returns
    ///  - `Block(pid)` if count == 0
    ///  - `Ok` otherwise
    pub fn wait(&mut self, pid: ProcessId) -> SemaphoreResult {
        if self.count == 0 {
            self.wait_queue.push_back(pid);
            return SemaphoreResult::Block(pid);
        } else {
            self.count -= 1;
            return SemaphoreResult::Ok;
        }
    }

    /// Signal the semaphore (release/up/verhogen)
    ///
    /// # Returns
    ///  - `WakeUp(pid)` if 'wait_queue' isn't empty
    ///  - `Ok` otherwise
    pub fn signal(&mut self) -> SemaphoreResult {
        if !self.wait_queue.is_empty() {
            let pid = self.wait_queue.pop_front().unwrap();
            return SemaphoreResult::WakeUp(pid);
        } else {
            self.count += 1;
            return SemaphoreResult::Ok;
        }
    }
}

#[derive(Debug, Default)]
pub struct SemaphoreSet {
    sems: BTreeMap<SemaphoreId, Mutex<Semaphore>>,
}

impl SemaphoreSet {
    // Insert a new semaphore into the sems
    pub fn insert(&mut self, key: u32, value: usize) -> bool {
        trace!("Sem Insert: <{:#x}>{}", key, value);

        let sid = SemaphoreId::new(key);
        let new_sem = Semaphore::new(value);
        self.sems
            .insert(sid, Mutex::new(new_sem))
            .is_none()
    }

    // Remove the semaphore from the sems
    pub fn remove(&mut self, key: u32) -> bool {
        trace!("Sem Remove: <{:#x}>", key);

        self.sems
            .remove(&SemaphoreId::new(key))
            .is_none()
    }

    /// Wait the semaphore (acquire/down/proberen)
    /// 
    /// # Returns
    ///  - 'wait' operation result if sems exist
    ///  - `NotExist` if the semaphore is not exist
    pub fn wait(&self, key: u32, pid: ProcessId) -> SemaphoreResult {
        let sid = SemaphoreId::new(key);

        // try get the semaphore from the sems
        //  then do it's operation        

        match self.sems.get(&sid) {
            Some(sem_lock) => sem_lock.lock().wait(pid),
            None => SemaphoreResult::NotExist,
        }
    }

    /// Signal the semaphore (release/up/verhogen)
    /// 
    /// # Returns
    ///  - 'signal' operation result if sems exist
    ///  - `NotExist` if the semaphore is not exist
    pub fn signal(&self, key: u32) -> SemaphoreResult {
        let sid = SemaphoreId::new(key);

        // try get the semaphore from the sems
        //  then do it's operation
        match self.sems.get(&sid) {
            Some(sem_lock) => sem_lock.lock().signal(),
            None => SemaphoreResult::NotExist,
        }
    }
}

impl core::fmt::Display for Semaphore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Semaphore({}) {:?}", self.count, self.wait_queue)
    }
}
