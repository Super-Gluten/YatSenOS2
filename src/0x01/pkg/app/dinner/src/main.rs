#![no_std]
#![no_main]

use lib::*;
use lib::sync::*;
use lib::sync::Semaphore;

extern crate lib;

const PHI_SIZE: usize = 5;
const CHOPSTICK: [Semaphore; 5] = semaphore_array![0, 1, 2, 3, 4];

// static S1: Semaphore = Semaphore::new(5);
// static S2: Semaphore = Semaphore::new(6);
static mut PHILOSOPHER: [i32; PHI_SIZE] = [0; PHI_SIZE];

fn main() -> isize {
    for i in 0..PHI_SIZE {
        CHOPSTICK[i].init(1);
    } // 初始化筷子信号量

    let help =r#"
        请选择使用的函数，它们的介绍如下：\n
        函数1：一般情况，会造成死锁。\n
        函数2：要求奇数号哲学家先拿左边的筷子，然后拿右边的筷子；偶数号哲学家相反。\n
        函数3：要求哲学家必须按照筷子从小到大拿去，会出现不公平甚至饥饿。\n
        请输入对应调用函数序号：
    "#;
    println!("{}", help);

    let mut pids: [u16; PHI_SIZE] = [0u16; PHI_SIZE];
    match stdin().read_line().trim() {
        "1" => {
            println!("函数1：一般情况，会造成死锁。");
            for i in 0..PHI_SIZE {
                let pid = sys_fork();
                if pid == 0 {
                    philosopher1(i);
                    sys_exit(0); 
                }
                pids[i] = pid;
            }
        }

        "2" => {
            println!("函数2：要求奇数号哲学家先拿左边的筷子，然后拿右边的筷子；偶数号哲学家相反。");
            for i in 0..PHI_SIZE {
                let pid = sys_fork();
                if pid == 0 {
                    philosopher2(i);
                    sys_exit(0);
                }
                pids[i] = pid;
            }
        }

        "3" => {
            println!("函数3：要求哲学家必须按照筷子从小到大拿去，会出现不公平甚至饥饿。");
            for i in 0..PHI_SIZE {
                let pid = sys_fork();
                if pid == 0 {
                    philosopher3(i);
                    sys_exit(0);
                }
                pids[i] = pid;
            }
        }
        _ => {
            println!("invaild input");
            for i in 0..PHI_SIZE {
                CHOPSTICK[i].remove();
            }
            return 0;
        }
    }
    let cpid = sys_get_pid();
    for i in 0..PHI_SIZE {
        println!("#{} is waiting for #{}", cpid, pids[i]);
        sys_wait_pid(pids[i]);
    }

    for i in 0..PHI_SIZE {
        CHOPSTICK[i].remove();
    }
    return 0;
}

const SLEEP_TIME: u64 = 2;
// 函数1：一般情况，会造成死锁。
fn philosopher1(i: usize){
    let left = i;
    let right = (i + 1) % PHI_SIZE;
    for _a in 0..20{
        //thinking
        CHOPSTICK[left].wait();
        println!("Philosopher {} get chopstick {}", i, left);
        sleep(SLEEP_TIME);
        CHOPSTICK[right].wait();
        println!("Philosopher {} get chopstick {}", i, right);
        sleep(SLEEP_TIME);
        //eating
        println!("\x1b[32mPhilosopher {} is eating\x1b[0m", i);
        CHOPSTICK[left].signal();
        println!("Philosopher {} release chopstick {}", i, left);
        sleep(SLEEP_TIME);
        CHOPSTICK[right].signal();
        println!("Philosopher {} release chopstick {}", i, right);
    }
}
// 函数2：要求奇数号哲学家先拿左边的筷子，然后拿右边的筷子；偶数号哲学家相反。不存在饥饿和死锁。
fn philosopher2(i: usize){
    let mut left = i;
    let mut right = (i + 1) % PHI_SIZE;
    for _a in 0..5{
        //thinking
        if i % 2 == 0 {
            left  = left ^ right;
            right = left ^ right;
            left  = left ^ right;
        } // 偶数号哲学家的左右可以认为是相反的
        CHOPSTICK[left].wait();
        println!("Philosopher {} get first chopstick {}", i, left);
        sleep(SLEEP_TIME);
        CHOPSTICK[right].wait();
        println!("Philosopher {} get second chopstick {}", i, right);
        sleep(SLEEP_TIME);
        //eating
        unsafe{
            PHILOSOPHER[i] += 1;
            println!("\x1b[32mPhilosopher {} is eating, he has eaten {} times.\x1b[0m", i, PHILOSOPHER[i]);
        }
        CHOPSTICK[left].signal();
        println!("Philosopher {} release chopstick {}", i, left);
        sleep(SLEEP_TIME);
        CHOPSTICK[right].signal();
        println!("Philosopher {} release chopstick {}", i, right);
    }
}

// 函数3：要求哲学家必须按照筷子从小到大拿去，会出现不公平甚至饥饿。
fn philosopher3(i: usize){
    let mut left = i;
    let mut right = (i + 1) % PHI_SIZE;
    for _a in 0..10{
        //thinking
        if left > right {
            left  = left ^ right;
            right = left ^ right;
            left  = left ^ right;
        }
        CHOPSTICK[left].wait();

        sleep(SLEEP_TIME);
        CHOPSTICK[right].wait();
        sleep(SLEEP_TIME);
        //eating
        unsafe{
            PHILOSOPHER[i] += 1;
            println!("\x1b[32mPhilosopher {} is eating, he has eaten {} times.\x1b[0m", i, PHILOSOPHER[i]);
        }
        
        CHOPSTICK[left].signal();
        sleep(SLEEP_TIME);
        CHOPSTICK[right].signal();
    }
}

entry!(main);