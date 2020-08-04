#[repr(transparent)]
pub struct BigEndianU16(u16);

impl BigEndianU16 {
    pub fn get(&self) -> u16 {
        self.0.swap_bytes()
    }
}

#[repr(transparent)]
pub struct BigEndianU32(u32);

impl BigEndianU32 {
    pub fn get(&self) -> u32 {
        self.0.swap_bytes()
    }
}

#[repr(transparent)]
pub struct BigEndianU64(u64);

impl BigEndianU64 {
    pub fn get(&self) -> u64 {
        self.0.swap_bytes()
    }
}

#[repr(transparent)]
pub struct BigEndianI16(i16);

impl BigEndianI16 {
    pub fn get(&self) -> i16 {
        self.0.swap_bytes()
    }
}

#[repr(transparent)]
pub struct BigEndianI32(i32);

impl BigEndianI32 {
    pub fn get(&self) -> i32 {
        self.0.swap_bytes()
    }
}

#[repr(transparent)]
pub struct BigEndianI64(i64);

impl BigEndianI64 {
    pub fn get(&self) -> i64 {
        self.0.swap_bytes()
    }
}
