use crate::errln;
use alloc::string::ToString;

#[macro_export]
macro_rules! entry {
    ($fn:ident) => {
        #[unsafe(export_name = "_start")]
        pub extern "C" fn __impl_start() {
            lib::init(); // THIS LINE IS NEW IN LAB 7
            let ret = $fn();
            // FIXME: after syscall, add lib::sys_exit(ret);
            lib::sys_exit(ret); // 调用syscall.rs中的sys_exit()
        }
    };
}

#[cfg_attr(not(test), panic_handler)]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let location = if let Some(location) = info.location() {
        alloc::format!(
            "{}@{}:{}",
            location.file(),
            location.line(),
            location.column()
        )
    } else {
        "Unknown location".to_string()
    };

    errln!(
        "\n\n\rERROR: panicked at {}\n\n\r{}",
        location,
        info.message()
    );

    crate::sys_exit(1);
}
