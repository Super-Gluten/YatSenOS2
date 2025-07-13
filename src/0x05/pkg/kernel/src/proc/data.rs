use alloc::{collections::BTreeMap, sync::Arc};
use spin::RwLock;
use x86_64::structures::paging::{
    Page,
    page::{PageRange, PageRangeInclusive},
};

use super::*;
use crate::utils::resource::ResourceSet;

#[derive(Debug, Clone)]
pub struct ProcessData {
    // shared data
    pub(super) env: Arc<RwLock<BTreeMap<String, String>>>,
    pub(super) resources: Arc<RwLock<ResourceSet>>, // 0x04 add
    pub(super) semaphores: Arc<RwLock<SemaphoreSet>>, // 0x05 add
}

impl Default for ProcessData {
    fn default() -> Self {
        Self {
            env: Arc::new(RwLock::new(BTreeMap::new())),
            resources: Arc::new(RwLock::new(ResourceSet::default())), // 0x04 add
            semaphores: Arc::new(RwLock::new(SemaphoreSet::default())),
        }
    }
}

impl ProcessData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn env(&self, key: &str) -> Option<String> {
        self.env.read().get(key).cloned()
    }

    pub fn set_env(&mut self, key: &str, val: &str) {
        self.env.write().insert(key.into(), val.into());
    }

    // 0x04 add: write() && read()
    pub fn read(&self, fd: u8, buf: &mut [u8]) -> isize {
        self.resources.read().read(fd, buf)
    }

    pub fn write(&self, fd: u8, buf: &[u8]) -> isize {
        self.resources.read().write(fd, buf)
    }

    // 0x05 add: 信号量的四种操作：
    pub fn sem_init(&mut self, key: u32, value: usize) -> bool {
        self.semaphores.write().insert(key, value)
    }

    pub fn sem_remove(&mut self, key: u32) -> bool {
        self.semaphores.write().remove(key)
    }

    pub fn sem_wait(&mut self, key: u32, pid: ProcessId) -> SemaphoreResult {
        self.semaphores.write().wait(key, pid)
    }
    pub fn sem_signal(&mut self, key: u32) -> SemaphoreResult {
        self.semaphores.write().signal(key)
    }
}
