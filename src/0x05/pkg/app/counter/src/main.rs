#![no_std]
#![no_main]

use lib::*;

extern crate lib;

const THREAD_COUNT: usize = 8;
static mut COUNTER: isize = 0;

// Define the key and the initial value of Semaphore
static SEM: Semaphore = Semaphore::new(0);
const SEM_VAL: usize = 1;

fn main() -> isize {
    let mut pids = [0u16; THREAD_COUNT];
    // don't forget to init Semaphore
    SEM.init(SEM_VAL);

    for i in 0..THREAD_COUNT {
        let pid = sys_fork();
        if pid == 0 {
            do_counter_inc();
            sys_exit(0);
        } else {
            pids[i] = pid; // only parent knows child's pid
        }
    }

    let cpid = sys_get_pid();
    println!("process #{} holds threads: {:?}", cpid, &pids);
    sys_stat();

    for i in 0..THREAD_COUNT {
        println!("#{} waiting for #{}...", cpid, pids[i]);
        sys_wait_pid(pids[i]);
    }

    // don't forget to remove the Semaphore
    SEM.remove();
    println!("COUNTER result: {}", unsafe { COUNTER });

    0
}

fn do_counter_inc() {
    for _ in 0..100 {
        SEM.wait();
        inc_counter();
        SEM.signal();
    }
}

/// Increment the counter
///
/// this function simulate a critical section by delay
/// DO NOT MODIFY THIS FUNCTION
fn inc_counter() {
    unsafe {
        delay();
        let mut val = COUNTER;
        delay();
        val += 1;
        delay();
        COUNTER = val;
    }
}

#[inline(never)]
#[unsafe(no_mangle)]
fn delay() {
    for _ in 0..0x100 {
        core::hint::spin_loop();
    }
}

entry!(main);
