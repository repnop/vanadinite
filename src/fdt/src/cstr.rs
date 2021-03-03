pub struct CStr<'a>(&'a [u8]);

impl<'a> CStr<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let end = data.iter().position(|&b| b == 0).unwrap();
        Self(&data[..end])
    }

    /// Does not include the null terminating byte
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_str(&self) -> Option<&'a str> {
        core::str::from_utf8(&self.0).ok()
    }
}
