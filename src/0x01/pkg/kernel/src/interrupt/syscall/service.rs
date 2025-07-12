use core::alloc::Layout;

use crate::proc::*;
use crate::utils::*;
use crate::memory::*;
use crate::proc;

use super::SyscallArgs;

// path: &str (ptr: arg0 as *const u8, len: arg1) -> pid: u16
pub fn spawn_process(args: &SyscallArgs) -> usize {
    // FIXME: get app name by args
    //       - core::str::from_utf8_unchecked
    //       - core::slice::from_raw_parts
    let name = unsafe {
        core::str::from_utf8_unchecked(
            core::slice::from_raw_parts(
                args.arg0 as *const u8, args.arg1
            )
        )
    };
    // FIXME: spawn the process by name
    // FIXME: handle spawn error, return 0 if failed
    // FIXME: return pid as usize
    match proc::spawn(name) {
        Some(pid) => return pid.0 as usize,
        _ => return 0,
    }
    
}

pub fn sys_write(args: &SyscallArgs) -> usize {
    // FIXME: get buffer and fd by args
    //       - core::slice::from_raw_parts
    let fd = args.arg0 as u8;
    let buf = unsafe{core::slice::from_raw_parts(args.arg1 as *const u8, args.arg2)};
    // FIXME: call proc::write -> isize
    // FIXME: return the result as usize
    proc::write(fd, buf) as usize
}

pub fn sys_read(args: &SyscallArgs) -> usize {
    // FIXME: just like sys_write
    let fd = args.arg0 as u8;
    let buf = unsafe{core::slice::from_raw_parts_mut(args.arg1 as *mut u8, args.arg2)};
    proc::read(fd, buf) as usize    
}

// ret: arg0 as isize
pub fn exit_process(args: &SyscallArgs, context: &mut ProcessContext) {
    // FIXME: exit process with retcode
    proc::process_exit(args.arg0 as isize, context);
}

pub fn list_process() {
    // FIXME: list all processes
    proc::print_process_list();
}

pub fn sys_allocate(args: &SyscallArgs) -> usize {
    let layout = unsafe { (args.arg0 as *const Layout).as_ref().unwrap() };

    if layout.size() == 0 {
        return 0;
    }

    let ret = crate::memory::user::USER_ALLOCATOR
        .lock()
        .allocate_first_fit(*layout);

    match ret {
        Ok(ptr) => ptr.as_ptr() as usize,
        Err(_) => 0,
    }
}

pub fn sys_deallocate(args: &SyscallArgs) {
    let layout = unsafe { (args.arg1 as *const Layout).as_ref().unwrap() };

    if args.arg0 == 0 || layout.size() == 0 {
        return;
    }

    let ptr = args.arg0 as *mut u8;

    unsafe {
        crate::memory::user::USER_ALLOCATOR
            .lock()
            .deallocate(core::ptr::NonNull::new_unchecked(ptr), *layout);
    }
}

// None -> pid: u16
pub fn sys_get_pid() -> u16 {
    proc::processor::get_pid().0
}

// pid: arg0 as u16 -> status: isize
pub fn sys_wait_pid(args: &SyscallArgs, context: &mut ProcessContext) {
    let pid = ProcessId(args.arg0 as u16);
    proc::wait_pid(pid, context);
}

pub fn list_app() {
    proc::list_app();
}

// None -> pid: u16 or 0 or -1
pub fn sys_fork(context: &mut ProcessContext){
    proc::fork(context);
}

// 0x05 add: 信号量的实现，根据args的值确定其不同操作
pub fn sys_sem(args: &SyscallArgs, context: &mut ProcessContext) {
    match args.arg0 {
        0 => context.set_rax(sem_init(args.arg1 as u32, args.arg2)),
        1 => context.set_rax(sem_remove(args.arg1 as u32)),
        2 => sem_signal(args.arg1 as u32, context),
        3 => sem_wait(args.arg1 as u32, context),
        _ => context.set_rax(usize::MAX),
    }
}

// 0x04 加分项, 0x05 add: sleep的实现
pub fn sys_time() -> u64 {
    let time = uefi::runtime::get_time().unwrap();
    let secs = time.hour() as u64 * 3600 + time.minute() as u64 * 60 + time.second() as u64;
    let msecs = time.nanosecond() / 1_000_000; 
    secs * 1000 + msecs as u64
}