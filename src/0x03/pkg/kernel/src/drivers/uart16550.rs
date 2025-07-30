const PORT: u16 = 0x3f8;
use bitflags::bitflags;
use core::fmt;
use x86_64::instructions::port::Port;

bitflags! {
    /// definition of flag bits for line control register
    pub struct LineControlFlags: u8 {
        /// 数据位长度: 00 = 5位, 01 = 6位, 10 = 7位, 11 = 8位
        const WORD_LENGTH_5 = 0b00;
        const WORD_LENGTH_6 = 0b01;
        const WORD_LENGTH_7 = 0b10;
        const WORD_LENGTH_8 = 0b11;

        /// 停止位数量 (1 = 2位, 0 = 1位)
        const STOP_BITS_1 = 0b0000_0000;
        const STOP_BITS_2 = 0b0000_0100;

        /// 奇偶校验使能 (1 = 启用)
        const PARITY_DISABLE = 0b0000_0000;
        const PARITY_ENABLE  = 0b0000_1000;

        /// 偶校验选择 (1 = 偶校验, 0 = 奇校验)
        const EVEN_PARITY = 0b0001_0000;

        /// 固定奇偶校验位 (1 = 强制校验位为0或1)
        const STICK_PARITY = 0b0010_0000;

        /// 中断控制 (1 = 强制发送线为逻辑0)
        const BREAK_CONTROL = 0b0100_0000;

        /// 分频器锁存访问位 (1 = 访问波特率分频器)
        const DLAB = 0b1000_0000;
    }
}

/// A port-mapped UART 16550 serial interface.
pub struct SerialPort {
    data: Port<u8>,
    interrupt_enable: Port<u8>,
    fifo_control: Port<u8>,
    line_control: Port<u8>,
    modem_control: Port<u8>,
    line_status: Port<u8>,
}
impl SerialPort {
    pub const fn new(_port: u16) -> Self {
        SerialPort {
            data: Port::new(PORT),
            interrupt_enable: Port::new(PORT + 1),
            fifo_control: Port::new(PORT + 2),
            line_control: Port::new(PORT + 3),
            modem_control: Port::new(PORT + 4),
            line_status: Port::new(PORT + 5),
        }
    }

    /// Initializes the serial port.
    pub fn init(&mut self) -> Result<(), &'static str> {
        unsafe {
            // 1. 禁用所有中断
            self.interrupt_enable.write(0x00);

            // 2. 启用DLAB(设置波特率除数)
            self.line_control.write(LineControlFlags::DLAB.bits());
            // self.line_control.write(0x80);

            // 3. 设置波特率除数(低位和高位)
            // 除数3对应38400波特率
            self.data.write(0x03); // 低位
            self.interrupt_enable.write(0x00); // 高位

            // 4. 设置线路控制寄存器: 8位数据, 无奇偶校验, 1位停止位
            self.line_control.write(
                LineControlFlags::WORD_LENGTH_8.bits()
                    | LineControlFlags::PARITY_DISABLE.bits()
                    | LineControlFlags::STOP_BITS_1.bits(),
            );
            // self.line_control.write(0x03);

            // 5. 启用FIFO, 清空缓冲区, 14字节阈值
            self.fifo_control.write(0xC7);

            // 6. 启用IRQ, RTS/DSR设置
            self.modem_control.write(0x0B);

            // 7. 设置为回环模式测试串口芯片
            self.modem_control.write(0x1E);

            // 8. 测试串口芯片(发送0xAE并检查是否返回相同字节)
            self.data.write(0xAE);

            // 检查串口是否故障(返回字节与发送的不同)
            if self.data.read() != 0xAE {
                return Err("Serial port faulty");
            }

            // 9. 设置为正常操作模式(非回环模式, 启用IRQ和OUT#1/OUT#2位)
            self.modem_control.write(0x0F);

            // 10. 为串口设备开启中断
            self.interrupt_enable.write(0x01);
        }

        Ok(())
    }

    /// Sends a byte on the serial port.
    pub fn send(&mut self, data: u8) {
        unsafe {
            while (self.line_status.read() & 0x20) == 0 {}
            self.data.write(data);
        }
    }

    /// Receives a byte on the serial port no wait.
    pub fn receive(&mut self) -> Option<u8> {
        unsafe {
            if self.line_status.read() & 1 != 0 {
                Some(self.data.read())
            } else {
                None
            }
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.send(byte);
        }
        Ok(())
    }
}
