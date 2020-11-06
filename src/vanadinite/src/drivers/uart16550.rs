const UART_DATA_REG_OFFSET: usize = 0;
const UART_INT_ENABLE_REG_OFFSET: usize = 1;
const UART_INT_ID_FIFO_CTRL_REG_OFFSET: usize = 2;
const UART_LINE_CTRL_REG_OFFSET: usize = 3;
const UART_MODEM_CTRL_REG_OFFSET: usize = 4;
const UART_LINE_STATUS_REG_OFFSET: usize = 5;
const UART_MODEM_STATUS_REG_OFFSET: usize = 6;
const UART_SCRATCH_REG_OFFSET: usize = 7;

#[repr(C)]
pub struct Uart16550 {
    data_register: u8,
    interrupt_enable: u8,
    int_id_fifo_control: u8,
    line_control: u8,
    modem_control: u8,
    line_status: u8,
    modem_status: u8,
    scratch: u8,
}

impl Uart16550 {
    pub fn init(&mut self) {
        self.interrupt_enable = 0x00;
        self.line_control = 0x80;
        self.data_register = 0x03;
        self.line_control = 0x03;
        self.int_id_fifo_control = 0xC7;
        self.modem_control = 0x0B;
    }

    pub fn line_status(&self) -> u8 {
        unsafe { (&self.line_status as *const u8).read_volatile() }
    }

    pub fn data_waiting(&self) -> bool {
        let value = self.line_status() & 1;

        value == 1
    }

    // TODO: error handling?
    pub fn read(&self) -> u8 {
        while !self.data_waiting() {}

        unsafe { (&self.data_register as *const u8).read_volatile() }
    }

    pub fn try_read(&self) -> Option<u8> {
        if !self.data_waiting() {
            return None;
        }

        Some(self.data_register)
    }

    pub fn data_empty(&self) -> bool {
        let value = self.line_status() & (1 << 6);

        value == (1 << 6)
    }

    pub fn write(&mut self, data: u8) {
        while !self.data_empty() {}

        unsafe {
            (&mut self.data_register as *mut u8).write_volatile(data);
        }
    }

    pub fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            self.write(byte);
        }
    }
}

impl core::fmt::Write for Uart16550 {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

impl crate::io::ConsoleDevice for Uart16550 {
    fn init(&mut self) {
        self.init();
    }

    fn read(&self) -> u8 {
        self.read()
    }

    fn write(&mut self, n: u8) {
        self.write(n)
    }
}

impl super::CompatibleWith for Uart16550 {
    fn list() -> &'static [&'static str] {
        &["ns16550", "ns16550a"]
    }
}
