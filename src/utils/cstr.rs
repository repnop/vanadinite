pub struct CStr(*const u8);

impl CStr {
    pub unsafe fn new(base: *const u8) -> Self {
        CStr(base)
    }

    /// Length of the string not including the null terminator
    pub fn len(&self) -> usize {
        self.bytes().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn bytes(&self) -> impl Iterator<Item = u8> {
        let mut ptr = self.0;
        let mut done = false;

        core::iter::from_fn(move || {
            if !done {
                let b = unsafe { ptr.read() };
                ptr = unsafe { ptr.add(1) };

                if b == 0 {
                    done = true;
                    return None;
                }

                return Some(b);
            }

            None
        })
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.0, self.len()) }
    }

    pub fn as_str(&self) -> Option<&str> {
        core::str::from_utf8(self.as_slice()).ok()
    }
}

impl core::fmt::Debug for CStr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CStr({:#p}) (\"", self.0)?;

        for byte in self.bytes() {
            write!(f, "{}", byte as char)?;
        }

        write!(f, "\")")
    }
}

impl Default for CStr {
    fn default() -> Self {
        static EMPTY_STR: u8 = 0;

        unsafe { Self::new(&EMPTY_STR as *const _) }
    }
}

impl core::cmp::PartialEq<[u8]> for CStr {
    fn eq(&self, other: &[u8]) -> bool {
        self.bytes().zip(other.iter().copied()).all(|(a, b)| a == b)
    }
}

impl core::cmp::PartialEq<&'_ [u8]> for CStr {
    fn eq(&self, other: &&[u8]) -> bool {
        self.bytes().zip(other.iter().copied()).all(|(a, b)| a == b)
    }
}

impl core::fmt::Display for CStr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_empty() {
            return write!(f, "<empty str>");
        }

        for byte in self.bytes() {
            write!(f, "{}", byte as char)?;
        }

        Ok(())
    }
}
