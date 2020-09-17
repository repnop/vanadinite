#[derive(Debug, Clone, Copy)]
pub struct Read;

#[derive(Debug, Clone, Copy)]
pub struct Write;

#[derive(Debug, Clone, Copy)]
pub struct Execute;

#[derive(Debug, Clone, Copy)]
pub struct ReadWrite;

#[derive(Debug, Clone, Copy)]
pub struct ReadExecute;

#[derive(Debug, Clone, Copy)]
pub struct ReadWriteExecute;

pub trait ToPermissions {
    fn to_permissions(self) -> Permissions;
}

impl ToPermissions for Read {
    fn to_permissions(self) -> Permissions {
        Permissions::Read
    }
}

impl ToPermissions for Execute {
    fn to_permissions(self) -> Permissions {
        Permissions::Execute
    }
}

impl ToPermissions for ReadExecute {
    fn to_permissions(self) -> Permissions {
        Permissions::ReadExecute
    }
}

impl ToPermissions for ReadWrite {
    fn to_permissions(self) -> Permissions {
        Permissions::ReadWrite
    }
}

impl ToPermissions for ReadWriteExecute {
    fn to_permissions(self) -> Permissions {
        Permissions::ReadWriteExecute
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum Permissions {
    Read = 0b001,
    Execute = 0b100,
    ReadWrite = 0b011,
    ReadExecute = 0b101,
    ReadWriteExecute = 0b111,
}

impl core::ops::BitOr<Execute> for Read {
    type Output = ReadExecute;

    fn bitor(self, _: Execute) -> Self::Output {
        ReadExecute
    }
}

impl core::ops::BitOr<Write> for Read {
    type Output = ReadWrite;

    fn bitor(self, _: Write) -> Self::Output {
        ReadWrite
    }
}

impl core::ops::BitOr<Read> for Execute {
    type Output = ReadExecute;

    fn bitor(self, _: Read) -> Self::Output {
        ReadExecute
    }
}

impl core::ops::BitOr<Read> for Write {
    type Output = ReadWrite;

    fn bitor(self, _: Read) -> Self::Output {
        ReadWrite
    }
}

impl core::ops::BitOr<Execute> for ReadWrite {
    type Output = ReadWriteExecute;

    fn bitor(self, _: Execute) -> Self::Output {
        ReadWriteExecute
    }
}

impl core::ops::BitOr<ReadWrite> for Execute {
    type Output = ReadWriteExecute;

    fn bitor(self, _: ReadWrite) -> Self::Output {
        ReadWriteExecute
    }
}
