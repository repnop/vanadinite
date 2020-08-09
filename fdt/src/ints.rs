#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct BigEndianU16(u16);

impl BigEndianU16 {
    pub fn get(&self) -> u16 {
        #[cfg(target_endian = "little")]
        return self.0.swap_bytes();

        #[cfg(target_endian = "big")]
        return self.0;
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct BigEndianU32(u32);

impl BigEndianU32 {
    pub fn get(&self) -> u32 {
        #[cfg(target_endian = "little")]
        return self.0.swap_bytes();

        #[cfg(target_endian = "big")]
        return self.0;
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct BigEndianU64(u64);

impl BigEndianU64 {
    pub fn get(&self) -> u64 {
        #[cfg(target_endian = "little")]
        return self.0.swap_bytes();

        #[cfg(target_endian = "big")]
        return self.0;
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct BigEndianI16(i16);

impl BigEndianI16 {
    pub fn get(&self) -> i16 {
        #[cfg(target_endian = "little")]
        return self.0.swap_bytes();

        #[cfg(target_endian = "big")]
        return self.0;
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct BigEndianI32(i32);

impl BigEndianI32 {
    pub fn get(&self) -> i32 {
        #[cfg(target_endian = "little")]
        return self.0.swap_bytes();

        #[cfg(target_endian = "big")]
        return self.0;
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct BigEndianI64(i64);

impl BigEndianI64 {
    pub fn get(&self) -> i64 {
        #[cfg(target_endian = "little")]
        return self.0.swap_bytes();

        #[cfg(target_endian = "big")]
        return self.0;
    }
}

macro_rules! implDebug {
    ($($t:ty),+) => {
        $(
            impl core::fmt::Debug for $t {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    core::fmt::Debug::fmt(&self.get(), f)
                }
            }
        )+
    };
}

implDebug!(BigEndianU16, BigEndianU32, BigEndianU64, BigEndianI16, BigEndianI32, BigEndianI64);
