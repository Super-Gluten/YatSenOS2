use chrono::{Duration, NaiveDateTime};
use chrono::{NaiveDate, NaiveTime};
use core::alloc::Layout;
use uefi::runtime::Time;

use crate::proc;
use crate::proc::*;

use super::SyscallArgs;

const SLEEP_TIME: i64 = 5;

// path: &str (ptr: arg0 as *const u8, len: arg1) -> pid: u16
pub fn spawn_process(args: &SyscallArgs) -> usize {
    //    1. get app name by args
    //       - core::str::from_utf8_unchecked
    //       - core::slice::from_raw_parts
    let name = unsafe {
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(
            args.arg0 as *const u8,
            args.arg1,
        ))
    };
    //     2. spawn the process by name
    //       - handle spawn error, return 0 if failed
    //       - return pid as usize
    match proc::spawn(name) {
        Some(pid) => return pid.0 as usize,
        _ => return 0,
    }
}

// fd: arg0 as u8, buf: &[u8] (ptr: arg1 as *const u8, len: arg2)
pub fn sys_write(args: &SyscallArgs) -> usize {
    //      get buffer and fd by args
    //      - use core::slice::from_raw_parts
    let fd = args.arg0 as u8;
    let buf = unsafe { core::slice::from_raw_parts(args.arg1 as *const u8, args.arg2) };
    //      call proc::write -> isize
    //      - return the result as usize
    proc::write(fd, buf) as usize
}

// fd: arg0 as u8, buf: &[u8] (ptr: arg1 as *const u8, len: arg2)
pub fn sys_read(args: &SyscallArgs) -> usize {
    //      get buffer and fd by args
    //      - use core::slice::from_raw_parts
    let fd = args.arg0 as u8;
    let buf = unsafe { core::slice::from_raw_parts_mut(args.arg1 as *mut u8, args.arg2) };
    //      call proc::read -> isize
    //      - return the result as usize
    proc::read(fd, buf) as usize
}

// ret: arg0 as isize
pub fn exit_process(args: &SyscallArgs, context: &mut ProcessContext) {
    proc::exit(args.arg0 as isize, context);
}

pub fn list_process() {
    proc::print_process_list();
}

// layout: arg0 as *const Layout -> ptr: *mut u8
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

// ptr: arg0 as *mut u8
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

pub fn sleep() {
    let start = uefi::runtime::get_time().unwrap();
    let dur = Duration::seconds(SLEEP_TIME);
    let mut current = start;
    while time_diff(start, current) < dur {
        current = uefi::runtime::get_time().unwrap();
    }
}

pub fn time_diff(start: Time, end: Time) -> Duration {
    let start_chrono = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(
            start.year().into(),
            start.month().into(),
            start.day().into(),
        )
        .unwrap(),
        NaiveTime::from_hms_nano_opt(
            start.hour().into(),
            start.minute().into(),
            start.second().into(),
            start.nanosecond().into(),
        )
        .unwrap(),
    );

    let end_chrono = NaiveDateTime::new(
        NaiveDate::from_ymd_opt(end.year().into(), end.month().into(), end.day().into()).unwrap(),
        NaiveTime::from_hms_nano_opt(
            end.hour().into(),
            end.minute().into(),
            end.second().into(),
            end.nanosecond().into(),
        )
        .unwrap(),
    );

    end_chrono - start_chrono
}

// None -> pid: u16 or 0 or -1
pub fn sys_fork(context: &mut ProcessContext) {
    proc::fork(context);
}

// op: u8, key: u32, val: usize -> ret: any
pub fn sys_sem(args: &SyscallArgs, context: &mut ProcessContext) {
    let op = args.arg0;
    let key = args.arg1;
    let value = args.arg2;

    match op {
        0 => context.set_rax(new_sem(key as u32, value)),
        1 => context.set_rax(remove_sem(key as u32)),
        2 => sem_signal(key as u32, context),
        3 => sem_wait(key as u32, context),
        _ => context.set_rax(usize::MAX),
    }
}