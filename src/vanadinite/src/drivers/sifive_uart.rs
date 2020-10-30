#[derive(Debug)]
#[repr(C)]
pub struct SiFiveUart {
    tx_data: [u8; 4],
    rx_data: [u8; 4],
    tx_control: u32,
    rx_control: u32,
    interrupt_enable: u32,
    interrupt_pending: u32,
    baud_rate_divisor: [u16; 2],
}

impl SiFiveUart {
    pub fn init(&mut self) {
        // Enable transmit
        self.tx_control |= 1;
        // Enable receive
        self.rx_control |= 1;
        // Disable interrupts
        self.interrupt_enable &= 0xFFFF_FFFC;
        // Set baud rate to 31250 Hz
        self.baud_rate_divisor[0] = 16000;
    }
    fn tx_full(&self) -> bool {
        let value = unsafe { (&self.tx_data as *const [u8; 4]).read_volatile() };

        value[3] & 0x80 == 0x80
    }

    fn rx_empty(&self) -> bool {
        let value = unsafe { (&self.rx_data as *const [u8; 4]).read_volatile() };

        value[3] & 0x80 == 0x80
    }

    pub fn read(&self) -> u8 {
        while self.rx_empty() {}

        self.rx_data[0]
    }

    pub fn write(&mut self, n: u8) {
        while self.tx_full() {}

        self.tx_data[0] = n;
    }
}
