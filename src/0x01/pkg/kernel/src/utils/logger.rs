use log::{Metadata, Record, Level, LevelFilter};

pub fn init() {
    static LOGGER: Logger = Logger;
    log::set_logger(&LOGGER).expect("Failed to set logger");

    // FIXME: Configure the logger

    // 在debug构建中，用trace记录包含详细调试信息的所有日志
    #[cfg(debug_assertions)]
    log::set_max_level(LevelFilter::Trace);

    // 在release构建中，只使用info记录重要日志
    #[cfg(not(debug_assertions))]
    log::set_max_level(LevelFilter::Info);

    // 输出日志系统初始化成功的消息
    info!("Logger Initialized.");
}

struct Logger;

impl log::Log for Logger {
    // 默认了日志级别都在允许的范围内（可修改？
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        // FIXME: Implement the logger with serial output
        
        // 使用self.enabled(record.metadata()) 
        // 来判断当前日志是否需要输出
        if !self.enabled(record.metadata()) {
            return ;
        }
        
        mod colors {
            pub const RED: &str = "\x1b[31m";
            pub const YELLOW: &str = "\x1b[33m";
            pub const GREEN: &str = "\x1b[32m";
            pub const DYAN: &str = "\x1b[36m"; // 青色
            pub const WHITE: &str = "\x1b[37m";
            pub const RESET: &str = "\x1b[0m";
        }

        let color_code = match record.level() {
            log::Level::Error => colors::RED ,
            log::Level::Warn => colors::YELLOW ,
            log::Level::Info => colors::GREEN ,
            log::Level::Debug => colors::DYAN ,
            log::Level::Trace => colors::WHITE ,
        };
        let reset_code = colors::RESET ;

        // 使用reco.file_static()和record.line()
        // 获取源文件的位置信息

        // println!格式为：[时间][级别名称][位置][模块]
        println!{
            "{}{:5}{} [{}{}][{}] {}",
            reset_code,
            record.level(),
            color_code,
            record.file_static().unwrap(),
            record.line().unwrap(),
            record.target(),
            record.args()
        };
    }

    fn flush(&self) {}
}
