#[derive(Debug)]
#[repr(C)]
pub enum SyscallNumbers {
    Exit = 0,
    Print = 1,
    ReadStdin = 2,
}

pub mod print {
    #[derive(Debug)]
    #[repr(C)]
    pub enum PrintErr {
        NoAccess,
    }
}
