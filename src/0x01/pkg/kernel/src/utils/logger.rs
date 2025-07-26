use log::{LevelFilter, Metadata, Record};
use boot::BootInfo;

pub fn init(boot_info: &'static BootInfo) {
    static LOGGER: Logger = Logger;
    log::set_logger(&LOGGER).expect("Failed to set logger");

    match boot_info.log_level {
        "Error" => {
            log::set_max_level(LevelFilter::Error);
        }
        "Warn" => {
            log::set_max_level(LevelFilter::Warn);
        }
        "info" => {
            log::set_max_level(LevelFilter::Info);
        }
        "debug" => {
            log::set_max_level(LevelFilter::Debug);
        }
        "trace" => {
            log::set_max_level(LevelFilter::Trace);
        }
        _ => {
            log::set_max_level(LevelFilter::Error);
            error!("the Logger LevelFilter is wrong!");
        }
    }
    info!("Logger Initialized.");
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    // handle serial output with different color
    //
    // control logs in different levels with function: enable()
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        mod colors {
            pub const RED: &str = "\x1b[31m";
            pub const YELLOW: &str = "\x1b[33m";
            pub const GREEN: &str = "\x1b[32m";
            pub const CYAN: &str = "\x1b[36m"; // 青色
            pub const WHITE: &str = "\x1b[37m";
            pub const RESET: &str = "\x1b[0m";
        }

        let color_code = match record.level() {
            log::Level::Error => colors::RED,
            log::Level::Warn => colors::YELLOW,
            log::Level::Info => colors::GREEN,
            log::Level::Debug => colors::CYAN,
            log::Level::Trace => colors::WHITE,
        };
        let reset_code = colors::RESET;

        println!(
            "{}{:5}{} [{}:{}] [{}]:  {}{:#?}{}",
            color_code,
            record.level(),
            reset_code,
            record.file().unwrap_or("unknown"), // 安全处理文件位置
            record.line().unwrap_or(0), // 安全处理文件行号
            record.target(), // 模块路径
            color_code,
            record.args(), // 日志具体内容
            reset_code,
        );
    }

    fn flush(&self) {}
}
