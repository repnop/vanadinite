#[derive(Debug)]
#[repr(C)]
pub enum Capability {
    None = 0,
    Driver = 1,
    Server = 2,
}

impl Default for Capability {
    fn default() -> Self {
        Capability::None
    }
}
