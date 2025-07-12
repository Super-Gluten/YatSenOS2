use syscall_def::Syscall;
use core::time::Duration;

#[inline(always)]
pub fn sys_write(fd: u8, buf: &[u8]) -> Option<usize> {
    let ret = syscall!(
        Syscall::Write,
        fd as u64,
        buf.as_ptr() as u64,
        buf.len() as u64
    ) as isize;
    if ret.is_negative() {
        None
    } else {
        Some(ret as usize)
    }
}

#[inline(always)]
pub fn sys_read(fd: u8, buf: &mut [u8]) -> Option<usize> {
    let ret = syscall!(
        Syscall::Read,
        fd as u64,
        buf.as_ptr() as u64,
        buf.len() as u64
    ) as isize;
    if ret.is_negative() {
        None
    } else {
        Some(ret as usize)
    }
}

#[inline(always)]
pub fn sys_wait_pid(pid: u16) -> isize {
    // FIXME: try to get the return value for process
    //        loop until the process is finished
    syscall!(Syscall::WaitPid, pid as u64) as isize
}

#[inline(always)]
pub fn sys_list_app() {
    syscall!(Syscall::ListApp);
}

#[inline(always)]
pub fn sys_stat() {
    syscall!(Syscall::Stat);
}

#[inline(always)]
pub fn sys_allocate(layout: &core::alloc::Layout) -> *mut u8 {
    syscall!(Syscall::Allocate, layout as *const _) as *mut u8
}

#[inline(always)]
pub fn sys_deallocate(ptr: *mut u8, layout: &core::alloc::Layout) -> usize {
    syscall!(Syscall::Deallocate, ptr, layout as *const _)
}

#[inline(always)]
pub fn sys_spawn(path: &str) -> u16 {
    syscall!(Syscall::Spawn, path.as_ptr() as u64, path.len() as u64) as u16
}

#[inline(always)]
pub fn sys_get_pid() -> u16 {
    syscall!(Syscall::GetPid) as u16
}

#[inline(always)]
pub fn sys_exit(code: isize) -> ! {
    syscall!(Syscall::Exit, code as u64);
    unreachable!("This process should be terminated by now.")
}

// 0x05 add
#[inline(always)]
pub fn sys_fork() -> u16 {
    syscall!(Syscall::Fork) as u16
}

// 0x05 add: 为四个信号操作分配系统调用
#[inline(always)]
pub fn sys_new_sem(key: u32, value: usize) -> bool {
    syscall!(Syscall::Sem, 0, key as usize, value) == 0
}

#[inline(always)]
pub fn sys_remove_sem(key: u32) -> bool {
    syscall!(Syscall::Sem, 1, key as usize) == 0
}

#[inline(always)]
pub fn sys_sem_signal(key: u32) -> bool {
    syscall!(Syscall::Sem, 2, key as usize) == 0
}

#[inline(always)]
pub fn sys_sem_wait(key: u32) -> bool {
    syscall!(Syscall::Sem, 3, key as usize) == 0
}

// 0x04 加分项，0x05 add：sleep的实现
#[inline(always)]
pub fn sys_time() -> u64 {
    syscall!(Syscall::Time) as u64
}

pub fn sleep(millisecs: u64) {
    let start = Duration::from_millis(sys_time());
    let dur = Duration::from_millis(millisecs as u64);
    let mut current = start;
    while current.saturating_sub(start) < dur {
        current = Duration::from_millis(sys_time());
    }
}