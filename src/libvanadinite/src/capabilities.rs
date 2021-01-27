#[derive(Debug)]
#[repr(C)]
pub enum Capability {
    None = 0,
    Driver = 1,
    Server = 2,
}
