pub fn test() -> ! {
    // 简单的无限循环计数器，发散函数
    let mut count = 0;
    let id;
    if let Some(id_env) = crate::proc::env("id") {
        id = id_env // 尝试从环境变量中获取id
    } else {
        id = "unknown".into() // 否则赋值为"unknown"
    }
    loop {
        // TODO: better way to show more than one process is running?
        count += 1;
        if count == 10000 {
            count = 0;
            print!("\r{:-6} => Tick!", id); // 每1000次循环，打印ID和 Tick信息
        }
        unsafe {
            x86_64::instructions::hlt();
        }
    }
}

#[inline(never)]
fn huge_stack() {
    println!("Huge stack testing...");

    let mut stack = [0u64; 0x1000];

    for (idx, item) in stack.iter_mut().enumerate() {
        *item = idx as u64;
    }

    for i in 0..stack.len() / 256 {
        println!("{:#05x} == {:#05x}", i * 256, stack[i * 256]);
    }
} // 用于测试栈的分配（栈是否大小足够且访问正确）

pub fn stack_test() -> ! {
    huge_stack();
    crate::proc::process_exit(0)
}
