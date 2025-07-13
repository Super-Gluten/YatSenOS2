#![no_std]
#![no_main]

use lib::sync::Semaphore;
use lib::*;

extern crate lib;

const THREAD_COUNT: usize = 16;
const MAX_MESSAGE_SIZE: usize = 10;
const QUEUE_SIZE: usize = MAX_MESSAGE_SIZE + 1;

struct MessageQueue {
    queue: [usize; QUEUE_SIZE],
    head: usize,
    tail: usize,
}

static mut MESSAGE_QUEUE: MessageQueue = MessageQueue {
    queue: [0; QUEUE_SIZE],
    head: 0,
    tail: 0,
};

static EMPTY: Semaphore = Semaphore::new(0);
static FULL: Semaphore = Semaphore::new(1);
static WRITE_MUTEX: Semaphore = Semaphore::new(2);

fn main() -> isize {
    let mut pids = [0u16; THREAD_COUNT];
    EMPTY.init(MAX_MESSAGE_SIZE); // 初始化empty=缓冲区大小
    FULL.init(0); // 初始化缓冲区为空
    WRITE_MUTEX.init(1); // 初始化写锁为1

    for i in 0..THREAD_COUNT {
        let pid = sys_fork();

        if i < THREAD_COUNT / 2 {
            if pid == 0 {
                for j in 0..MAX_MESSAGE_SIZE {
                    write_message(i + j);
                }
                sys_exit(0);
            } else {
                pids[i] = pid;
            }
        } else {
            if pid == 0 {
                for _ in 0..MAX_MESSAGE_SIZE {
                    read_message();
                }
                sys_exit(0);
            } else {
                pids[i] = pid;
            }
        }
    }

    let cpid = sys_get_pid();
    println!("process #{} holds threads: {:?}", cpid, &pids);
    sys_stat();

    for i in 0..THREAD_COUNT {
        println!("#{} waiting for #{}...", cpid, pids[i]);
        sys_wait_pid(pids[i]);
    }

    println!("Message Queue: {:?}", unsafe { MESSAGE_QUEUE.queue });

    return 0;
}

fn write_message(message: usize) {
    unsafe {
        EMPTY.wait();
        WRITE_MUTEX.wait();
        MESSAGE_QUEUE.queue[MESSAGE_QUEUE.tail] = message;
        MESSAGE_QUEUE.tail = (MESSAGE_QUEUE.tail + 1) % QUEUE_SIZE;
        WRITE_MUTEX.signal();
        FULL.signal();
    }
}

fn read_message() {
    unsafe {
        FULL.wait();
        WRITE_MUTEX.wait();
        MESSAGE_QUEUE.queue[MESSAGE_QUEUE.head] = 0;
        MESSAGE_QUEUE.head = (MESSAGE_QUEUE.head + 1) % QUEUE_SIZE;
        WRITE_MUTEX.signal();
        EMPTY.signal();
    }
}

entry!(main);
