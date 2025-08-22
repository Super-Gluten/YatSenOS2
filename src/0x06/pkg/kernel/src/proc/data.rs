use super::*;
use crate::utils::resource::ResourceSet;
use sync::SemaphoreSet;
use alloc::{collections::BTreeMap, sync::Arc};
use spin::RwLock;

#[derive(Debug, Clone)]
pub struct ProcessData {
    // shared data
    pub(super) env: Arc<RwLock<BTreeMap<String, String>>>,
    pub(super) resources: Arc<RwLock<ResourceSet>>,
    pub(super) semaphores: Arc<RwLock<SemaphoreSet>>,
}

impl Default for ProcessData {
    fn default() -> Self {
        Self {
            env: Arc::new(RwLock::new(BTreeMap::new())),
            resources: Arc::new(RwLock::new(ResourceSet::default())),
            semaphores: Arc::new(RwLock::new(SemaphoreSet::default()))
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

    pub fn read(&self, fd: u8, buf: &mut [u8]) -> isize {
        self.resources.read().read(fd, buf)
    }

    pub fn write(&self, fd: u8, buf: &[u8]) -> isize {
        self.resources.read().write(fd, buf)
    }

    pub fn new_sem(&mut self, key: u32, value: usize) -> usize {
        if self.semaphores.write().insert(key, value) {
            0
        } else {
            1
        }
    }

    pub fn remove_sem(&mut self, key: u32) -> usize {
        if self.semaphores.write().remove(key) {
            0
        } else {
            1
        }
    }

    pub fn sem_signal(&mut self, key: u32) -> SemaphoreResult {
        self.semaphores.write().signal(key)
    }

    pub fn sem_wait(&mut self, key: u32, pid: ProcessId) -> SemaphoreResult {
        self.semaphores.write().wait(key, pid)
    }
}
