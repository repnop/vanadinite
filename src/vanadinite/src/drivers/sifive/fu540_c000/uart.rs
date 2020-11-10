#[derive(Debug)]
#[repr(C)]
pub struct SiFiveUart {
    tx_data: registers::TxData,
    rx_data: registers::RxData,
    tx_control: registers::TxCtrl,
    rx_control: Volatile<u32>,
    interrupt_enable: Volatile<u32>,
    interrupt_pending: Volatile<u32>,
    baud_rate_divisor: Volatile<[u16; 2]>,
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
        self.baud_rate_divisor[0].write(16000);
    }
    fn tx_full(&self) -> bool {
        let value = self.tx_data.read();

        value[3] & 0x80 == 0x80
    }

    fn rx_empty(&self) -> bool {
        let value = self.rx_data.read();

        value[3] & 0x80 == 0x80
    }

    pub fn read(&self) -> u8 {
        while self.rx_empty() {}

        self.rx_data[0].read()
    }

    pub fn write(&mut self, n: u8) {
        while self.tx_full() {}

        self.tx_data[0].write(n);
    }
}

mod registers {
    use crate::utils::Volatile;
    #[derive(Debug)]
    #[repr(transparent)]
    pub struct TxData(Volatile<[u8; 4]>);

    impl TxData {
        pub fn write(&mut self, val: u8) {
            self.0[0].write(val);
        }

        pub fn is_full(&self) -> bool {
            self.0[3].read() >> 7 == 1
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct RxData(Volatile<[u8; 4]>);

    impl RxData {
        pub fn read(&mut self) -> u8 {
            self.0[0].read()
        }

        pub fn is_empty(&self) -> bool {
            self.0[3].read() >> 7 == 1
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct TxCtrl(Volatile<[u8; 4]>);

    impl TxCtrl {
        pub fn tx_enable(&mut self, enable: bool) {
            let val = self.0[0].read() | (enable as u8);
            self.0[0].write(val);
        }

        pub fn stop_bit(&mut self, enable: bool) {
            let val = self.0[0].read() | ((enable as u8) << 1);
            self.0[0].write(val);
        }
    }
}
