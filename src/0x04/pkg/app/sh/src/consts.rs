pub const SIGN: [&str; 5] = [
    r#"__  __      __  _____            ____  _____"#,
    r#"\ \/ /___ _/ /_/ ___/___  ____  / __ \/ ___/"#,
    r#" \  / __ `/ __/\__ \/ _ \/ __ \/ / / /\__ \"#,
    r#" / / /_/ / /_ ___/ /  __/ / / / /_/ /___/ /"#,
    r#"/_/\__,_/\__//____/\___/_/ /_/\____//____/"#,
];

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
// pub const BLINK: &str = "\x1b[5m";
// pub const DIM: &str = "\x1b[2m"; // 暗淡效果，用于制造阴影
pub const R1: &str = "\x1b[91m"; // 亮红
pub const R2: &str = "\x1b[93m"; // 亮黄
pub const R3: &str = "\x1b[92m"; // 亮绿
pub const R4: &str = "\x1b[96m"; // 亮青
pub const R5: &str = "\x1b[94m"; // 亮蓝
pub const R6: &str = "\x1b[95m"; // 亮洋红
pub const RAINBOW: [&str; 6] = [R1, R2, R3, R4, R5, R6];

// 学号和姓名行
pub const STUDENT_INFO: &str = "xxxxxxxx xxx";
