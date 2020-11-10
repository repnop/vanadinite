use crate::utils::Volatile;

#[derive(Debug)]
#[repr(C)]
pub struct Clint {
    msip0: Volatile<u32>,
    msip1: Volatile<u32>,
    msip2: Volatile<u32>,
    msip3: Volatile<u32>,
    msip4: Volatile<u32>,
    _reserved1: [u8; 16364],
    mtimecmp0: Volatile<u64>,
    mtimecmp1: Volatile<u64>,
    mtimecmp2: Volatile<u64>,
    mtimecmp3: Volatile<u64>,
    mtimecmp4: Volatile<u64>,
    _reserved2: [u8; 32720],
    mtime: Volatile<u64>,
}