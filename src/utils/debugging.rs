pub struct DebugBytesAt(*const u8);

impl DebugBytesAt {
    pub unsafe fn new(ptr: *const u8) -> Self {
        Self(ptr)
    }
}

impl core::fmt::Display for DebugBytesAt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#p}: ", self.0)?;
        let mut chars = [' '; 16];
        let as_array: &[u8; 16] = unsafe { &*(self.0.cast()) };

        for (i, byte) in as_array.iter().copied().enumerate() {
            if byte >= 32 && byte <= 127 {
                chars[i] = byte as char;
            }

            write!(f, "{:0>2x} ", byte)?;
        }

        write!(f, "  |  ")?;

        for c in chars.iter() {
            write!(f, "{}", c)?;
        }

        Ok(())
    }
}
