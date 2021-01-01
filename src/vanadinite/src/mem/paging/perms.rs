// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

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

#[derive(Debug, Clone, Copy)]
pub struct User;

#[derive(Debug, Clone, Copy)]
pub struct UserRead;

#[derive(Debug, Clone, Copy)]
pub struct UserWrite;

#[derive(Debug, Clone, Copy)]
pub struct UserExecute;

#[derive(Debug, Clone, Copy)]
pub struct UserReadWrite;

#[derive(Debug, Clone, Copy)]
pub struct UserReadExecute;

#[derive(Debug, Clone, Copy)]
pub struct UserReadWriteExecute;

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

impl ToPermissions for UserRead {
    fn to_permissions(self) -> Permissions {
        Permissions::UserRead
    }
}

impl ToPermissions for UserExecute {
    fn to_permissions(self) -> Permissions {
        Permissions::UserExecute
    }
}

impl ToPermissions for UserReadExecute {
    fn to_permissions(self) -> Permissions {
        Permissions::UserReadExecute
    }
}

impl ToPermissions for UserReadWrite {
    fn to_permissions(self) -> Permissions {
        Permissions::UserReadWrite
    }
}

impl ToPermissions for UserReadWriteExecute {
    fn to_permissions(self) -> Permissions {
        Permissions::UserReadWriteExecute
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum Permissions {
    Read = 0b0001,
    Execute = 0b0100,
    ReadWrite = 0b0011,
    ReadExecute = 0b0101,
    ReadWriteExecute = 0b0111,
    User = 0b1000,
    UserRead = 0b1001,
    UserExecute = 0b1100,
    UserReadWrite = 0b1011,
    UserReadExecute = 0b1101,
    UserReadWriteExecute = 0b1111,
}

impl ToPermissions for Permissions {
    fn to_permissions(self) -> Permissions {
        self
    }
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

impl core::ops::BitOr<User> for Read {
    type Output = UserRead;

    fn bitor(self, _: User) -> Self::Output {
        UserRead
    }
}

impl core::ops::BitOr<User> for Write {
    type Output = UserWrite;

    fn bitor(self, _: User) -> Self::Output {
        UserWrite
    }
}

impl core::ops::BitOr<User> for Execute {
    type Output = UserExecute;

    fn bitor(self, _: User) -> Self::Output {
        UserExecute
    }
}

impl core::ops::BitOr<Read> for User {
    type Output = UserRead;

    fn bitor(self, _: Read) -> Self::Output {
        UserRead
    }
}

impl core::ops::BitOr<Write> for User {
    type Output = UserWrite;

    fn bitor(self, _: Write) -> Self::Output {
        UserWrite
    }
}

impl core::ops::BitOr<Execute> for User {
    type Output = UserExecute;

    fn bitor(self, _: Execute) -> Self::Output {
        UserExecute
    }
}

impl core::ops::BitOr<User> for ReadWrite {
    type Output = UserReadWrite;

    fn bitor(self, _: User) -> Self::Output {
        UserReadWrite
    }
}

impl core::ops::BitOr<User> for ReadWriteExecute {
    type Output = UserReadWriteExecute;

    fn bitor(self, _: User) -> Self::Output {
        UserReadWriteExecute
    }
}

impl core::ops::BitOr<ReadWrite> for User {
    type Output = UserReadWrite;

    fn bitor(self, _: ReadWrite) -> Self::Output {
        UserReadWrite
    }
}

impl core::ops::BitOr<ReadWriteExecute> for User {
    type Output = UserReadWriteExecute;

    fn bitor(self, _: ReadWriteExecute) -> Self::Output {
        UserReadWriteExecute
    }
}

impl core::ops::BitOr<UserWrite> for Read {
    type Output = UserReadWrite;

    fn bitor(self, _: UserWrite) -> Self::Output {
        UserReadWrite
    }
}

impl core::ops::BitOr<UserRead> for Write {
    type Output = UserReadWrite;

    fn bitor(self, _: UserRead) -> Self::Output {
        UserReadWrite
    }
}

impl core::ops::BitOr<UserRead> for Execute {
    type Output = UserReadExecute;

    fn bitor(self, _: UserRead) -> Self::Output {
        UserReadExecute
    }
}

impl core::ops::BitOr<Write> for UserRead {
    type Output = UserReadWrite;

    fn bitor(self, _: Write) -> Self::Output {
        UserReadWrite
    }
}

impl core::ops::BitOr<Execute> for UserRead {
    type Output = UserReadExecute;

    fn bitor(self, _: Execute) -> Self::Output {
        UserReadExecute
    }
}
