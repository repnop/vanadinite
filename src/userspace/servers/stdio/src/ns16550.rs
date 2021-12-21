use volatile::Volatile;

#[repr(C)]
pub struct Uart16550 {
    data_register: Volatile<u8>,
    interrupt_enable: Volatile<u8>,
    int_id_fifo_control: Volatile<u8>,
    line_control: Volatile<u8>,
    modem_control: Volatile<u8>,
    line_status: Volatile<u8>,
    modem_status: Volatile<u8>,
    scratch: Volatile<u8>,
}

impl Uart16550 {
    pub fn init(&self) {
        self.line_control.write(0x03);
        self.int_id_fifo_control.write(0x01);
        self.interrupt_enable.write(0x01);

        let lcr = self.line_control.read();
        self.line_control.write(lcr | (1 << 7));

        // Full speed, baybee
        self.data_register.write(1);
        self.interrupt_enable.write(0);

        self.line_control.write(lcr);

        self.scratch.write(0);
    }

    pub fn line_status(&self) -> u8 {
        self.line_status.read()
    }

    pub fn data_waiting(&self) -> bool {
        let value = self.line_status() & 1;

        value == 1
    }

    // TODO: error handling?
    pub fn read(&self) -> u8 {
        while !self.data_waiting() {}

        self.data_register.read()
    }

    pub fn try_read(&self) -> Option<u8> {
        if !self.data_waiting() {
            return None;
        }

        Some(self.data_register.read())
    }

    pub fn data_empty(&self) -> bool {
        let value = self.line_status() & (1 << 5);

        value == (1 << 5)
    }

    pub fn write(&self, data: u8) {
        while !self.data_empty() {}

        if data == 127 {
            self.write_str("\x1B[1D \x1B[1D");
        }

        self.data_register.write(data);
    }

    pub fn write_str(&self, s: &str) {
        for byte in s.bytes() {
            self.write(byte);
        }
    }
}
