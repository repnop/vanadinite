pub(crate) struct Stdout;

impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        crate::syscalls::print(s.as_bytes());
        Ok(())
    }
}
