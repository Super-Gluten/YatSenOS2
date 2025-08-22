#![no_std]
#![no_main]

use alloc::string::{String, ToString};
use lib::*;

extern crate lib;

const FISH_EXAMPLE: &str = "<><_";
const THREAD_COUNT: usize = 3;
const FISH_SIZE: usize = 10;
const RED: &str = "\x1b[91m"; // 亮红
const GREEN: &str = "\x1b[92m"; // 亮绿
const BLUE: &str = "\x1b[94m"; // 亮蓝
const RESET: &str = "\x1b[0m";
const COLOR_MODE: [&str; THREAD_COUNT] = [RED, GREEN, BLUE];

static SEM_HEAD: Semaphore = Semaphore::new(0);
static SEM_BODY: Semaphore = Semaphore::new(1);
static SEM_TAIL: Semaphore = Semaphore::new(2);
static SEM_WAVE: Semaphore = Semaphore::new(3);
static WRITE_MUTEX: Semaphore = Semaphore::new(4);
const SEM_VAL: usize = 1;

fn main() -> isize {
    let mut pids = [0u16; THREAD_COUNT];

    SEM_HEAD.init(SEM_VAL);
    SEM_BODY.init(0);
    SEM_TAIL.init(0);
    SEM_WAVE.init(0);
    WRITE_MUTEX.init(1);

    for i in 0..THREAD_COUNT {
        let pid = sys_fork();
        if pid == 0 {
            for _j in 0..FISH_SIZE {
                fish_print(i);
            }
            sys_exit(0);
        } else {
            pids[i] = pid;
        }
    }

    let cpid = sys_get_pid();
    WRITE_MUTEX.wait();
    println!("process #{} holds threads: {:?}", cpid, &pids);
    sys_stat();
    WRITE_MUTEX.signal();

    for i in 0..THREAD_COUNT {
        println!("#{} waiting for #{}...", cpid, pids[i]);
        sys_wait_pid(pids[i]);
    }


    SEM_HEAD.remove();
    SEM_BODY.remove();
    SEM_TAIL.remove();
    SEM_WAVE.remove();
    WRITE_MUTEX.remove();
    
    0
}

fn fish_head() -> String {
    SEM_HEAD.wait();
    let result = format!("<");
    SEM_BODY.signal();
    result
}

fn fish_body() -> String {
    SEM_BODY.wait();
    let result = format!(">");
    SEM_TAIL.signal();
    result
}

fn fish_tail() -> String {
    SEM_TAIL.wait();
    let result = format!("<");
    SEM_WAVE.signal();
    result
}

fn fish_wave() -> String {
    SEM_WAVE.wait();
    let result = format!("_ ");
    SEM_HEAD.signal();
    result
}

fn fish_print(color: usize) {
    let head = fish_head();
    let body = fish_body();
    let tail = fish_tail();
    let wave = fish_wave();
    
    WRITE_MUTEX.wait();
    print!("{}{}{}{}{}{}",
    COLOR_MODE[color], head, body, tail, wave, RESET);
    WRITE_MUTEX.signal();
}
entry!(main);
