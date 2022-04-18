// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod channel;
pub mod io;
pub mod mem;
pub mod vmspace;
pub mod task;

use crate::error::{SyscallError, RawSyscallError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum Syscall {
    Exit = 0,
    Print = 1,
    ReadStdin = 2,
    ReadMessage = 3,
    AllocVirtualMemory = 4,
    GetTid = 5,
    ReadChannel = 7,
    CreateChannelMessage = 8,
    SendChannelMessage = 9,
    RetireChannelMessage = 10,
    AllocDmaMemory = 12,
    CreateVmspace = 13,
    AllocVmspaceObject = 14,
    SpawnVmspace = 15,
    ClaimDevice = 16,
    QueryMemoryCapability = 20,
    CompleteInterrupt = 21,
    QueryMmioCapability = 22,
    ReadChannelNonBlocking = 23,
}

impl Syscall {
    pub fn from_usize(n: usize) -> Option<Self> {
        match n {
            0 => Some(Self::Exit),
            1 => Some(Self::Print),
            2 => Some(Self::ReadStdin),
            3 => Some(Self::ReadMessage),
            4 => Some(Self::AllocVirtualMemory),
            5 => Some(Self::GetTid),
            7 => Some(Self::ReadChannel),
            8 => Some(Self::CreateChannelMessage),
            9 => Some(Self::SendChannelMessage),
            10 => Some(Self::RetireChannelMessage),
            12 => Some(Self::AllocDmaMemory),
            13 => Some(Self::CreateVmspace),
            14 => Some(Self::AllocVmspaceObject),
            15 => Some(Self::SpawnVmspace),
            16 => Some(Self::ClaimDevice),
            20 => Some(Self::QueryMemoryCapability),
            21 => Some(Self::CompleteInterrupt),
            22 => Some(Self::QueryMmioCapability),
            23 => Some(Self::ReadChannelNonBlocking),
            _ => None,
        }
    }
}

pub unsafe fn syscall0r0(id: Syscall) -> Result<(), SyscallError> {
    let error: Option<RawSyscallError>;
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") id as usize => error,
        );
    }

    match error {
        Some(error) => Err(error.cook()),
        None => Ok(())
    }
}

pub unsafe fn syscall0r1(id: Syscall) -> Result<usize, SyscallError> {
    let error: Option<RawSyscallError>;
    let ret0: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") id as usize => error,
            lateout("a1") ret0, 
        );
    }

    match error {
        Some(error) => Err(error.cook()),
        None => Ok(ret0)
    }
}

pub unsafe fn syscall1r3(id: Syscall, arg0: usize) -> Result<(usize, usize, usize), SyscallError> {
    let error: Option<RawSyscallError>;
    let ret0: usize;
    let ret1: usize;
    let ret2: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") id as usize => error,
            inlateout("a1") arg0 => ret0,
            lateout("a2") ret1,
            lateout("a3") ret2,
        );
    }

    match error {
        Some(error) => Err(error.cook()),
        None => Ok((ret0, ret1, ret2))
    }
}

pub unsafe fn syscall2r0(id: Syscall, arg0: usize, arg1: usize) -> Result<(), SyscallError> {
    let error: Option<RawSyscallError>;
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") id as usize => error,
            inlateout("a1") arg0 => _,
            inlateout("a2") arg1 => _,
        );
    }

    match error {
        Some(error) => Err(error.cook()),
        None => Ok(())
    }
}
