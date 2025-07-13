use core::{
    hint::spin_loop,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::*;

pub struct SpinLock {
    bolt: AtomicBool,
}

impl SpinLock {
    pub const fn new() -> Self {
        Self {
            bolt: AtomicBool::new(false),
        }
    }

    pub fn acquire(&self) {
        // FIXME: acquire the lock, spin if the lock is not available
        while self
            .bolt
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            == Err(true)
        {
            core::hint::spin_loop(); // 这里采用的是自旋等待，告知CPU其处于忙等待状态
        } // 原子性检查锁是否处于锁定态，未锁定的情况下更新为锁定态。 
    }

    pub fn release(&self) {
        // FIXME: release the lock
        self.bolt.store(false, Ordering::SeqCst);
    } // 原子性设置为未锁定态
}

unsafe impl Sync for SpinLock {} // Why? Check reflection question 5

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Semaphore {
    /* FIXME: record the sem key */
    key: u32,
}

impl Semaphore {
    pub const fn new(key: u32) -> Self {
        Semaphore { key }
    }

    #[inline(always)]
    pub fn init(&self, value: usize) -> bool {
        sys_new_sem(self.key, value)
    } // new操作

    /* FIXME: other functions with syscall... */
    // 添加信号量所需的其他三种操作
    #[inline(always)]
    pub fn remove(&self) -> bool {
        sys_remove_sem(self.key)
    } // remove操作

    #[inline(always)]
    pub fn wait(&self) -> bool {
        sys_sem_wait(self.key)
    } // P操作

    pub fn signal(&self) -> bool {
        sys_sem_signal(self.key)
    } // V操作
}

unsafe impl Sync for Semaphore {}

#[macro_export]
macro_rules! semaphore_array {
    [$($x:expr),+ $(,)?] => {
        [ $($crate::Semaphore::new($x),)* ]
    }
}
