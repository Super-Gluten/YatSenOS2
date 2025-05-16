const PORT: u16 = 0x3f8;
use core::fmt;
use x86_64::instructions::port::Port;

/// A port-mapped UART 16550 serial interface.
pub struct SerialPort {
    data: Port<u8>,
    interrupt_enable: Port<u8>,
    fifo_ctrl: Port<u8>,
    line_ctrl: Port<u8>,
    modem_ctrl: Port<u8>,
    line_status: Port<u8>,
}

impl SerialPort {
    pub const fn new(_port: u16) -> Self {
        SerialPort {
            data: Port::new(PORT),
            interrupt_enable: Port::new(PORT + 1),
            fifo_ctrl: Port::new(PORT + 2),
            line_ctrl: Port::new(PORT + 3),
            modem_ctrl: Port::new(PORT + 4),
            line_status: Port::new(PORT + 5),
        }
    }

    /// Initializes the serial port.
    // 更改为了可变借用
    pub fn init(&mut self) -> Result<(), &'static str> {
        // FIXME: Initialize the serial port
        unsafe {
            // 1. 禁用所有中断
            self.interrupt_enable.write(0x00);

            // 2. 启用DLAB(设置波特率除数)
            self.line_ctrl.write(0x80);

            // 3. 设置波特率除数(低位和高位)
            // 除数3对应38400波特率
            self.data.write(0x03); // 低位
            self.interrupt_enable.write(0x00); // 高位

            // 4. 设置线路控制寄存器: 8位数据, 无奇偶校验, 1位停止位
            self.line_ctrl.write(0x03);

            // 5. 启用FIFO, 清空缓冲区, 14字节阈值
            self.fifo_ctrl.write(0xC7);

            // 6. 启用IRQ, RTS/DSR设置
            self.modem_ctrl.write(0x0B);

            // 7. 设置为回环模式测试串口芯片
            self.modem_ctrl.write(0x1E);

            // 8. 测试串口芯片(发送0xAE并检查是否返回相同字节)
            self.data.write(0xAE);

            // 检查串口是否故障(返回字节与发送的不同)
            if self.data.read() != 0xAE {
                return Err("Serial port faulty");
            }

            // 9. 设置为正常操作模式(非回环模式, 启用IRQ和OUT#1/OUT#2位)
            self.modem_ctrl.write(0x0F);

            // 10. 为串口设备开启中断
            self.interrupt_enable.write(0x01);
        }

        Ok(())
    }

    /// Sends a byte on the serial port.
    pub fn send(&mut self, data: u8) {
        // FIXME: Send a byte on the serial port
        unsafe {
            while (self.line_status.read() & 0x20) == 0 {}
            self.data.write(data);
        }
    }

    /// Receives a byte on the serial port no wait.
    pub fn receive(&mut self) -> Option<u8> {
        // FIXME: Receive a byte on the serial port no wait
        unsafe {
            if self.line_status.read() & 1 != 0 {
                // 数据就绪，读取并返回一个字节
                Some(self.data.read())
            } else {
                // 否则返回none
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
