// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod channel;
pub mod io;
pub mod mem;
pub mod task;
pub mod vmspace;

use crate::error::{RawSyscallError, SyscallError};

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
        None => Ok(()),
    }
}

pub unsafe fn syscall1r0(id: Syscall, arg0: usize) -> Result<(), SyscallError> {
    let error: Option<RawSyscallError>;
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") id as usize => error,
            in("a1") arg0,
        );
    }

    match error {
        Some(error) => Err(error.cook()),
        None => Ok(()),
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
        None => Ok(ret0),
    }
}

pub unsafe fn syscall1r1(id: Syscall, arg0: usize) -> Result<usize, SyscallError> {
    let error: Option<RawSyscallError>;
    let ret0: usize;

    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") id as usize => error,
            inlateout("a1") arg0 => ret0,
        );
    }

    match error {
        Some(error) => Err(error.cook()),
        None => Ok(ret0),
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
        None => Ok((ret0, ret1, ret2)),
    }
}

pub unsafe fn syscall2r0(id: Syscall, arg0: usize, arg1: usize) -> Result<(), SyscallError> {
    let error: Option<RawSyscallError>;
    unsafe {
        core::arch::asm!(
            "ecall",
            inlateout("a0") id as usize => error,
            in("a1") arg0,
            in("a2") arg1,
        );
    }

    match error {
        Some(error) => Err(error.cook()),
        None => Ok(()),
    }
}

#[macro_export]
macro_rules! syscall {
    ($syscall:expr) => {{
        let error: core::option::Option<$crate::error::RawSyscallError>;
        let syscall: $crate::syscalls::Syscall = $syscall;

        core::arch::asm!(
            "ecall",
            inlateout("a0") syscall as usize => error
        );

        match error {
            core::option::Option::Some(error) => core::result::Result::Err(error.cook()),
            None => core::result::Result::Ok(())
        }
    }};

    ($syscall:expr, $($arms:tt)*) => {{
        let error: core::option::Option<$crate::error::RawSyscallError>;
        let syscall: $crate::syscalls::Syscall = $syscall;

        core::arch::asm!(
            "ecall",
            inlateout("a0") syscall as usize => error,
            crate::syscall!(@genarms @inout $($arms:tt)*),
        );

        match error {
            core::option::Option::Some(error) => core::result::Result::Err(error.cook()),
            None => core::result::Result::Ok(())
        }
    }};

    (@genarms @inout 1 $in:expr => $out:expr, $($arms:tt)*) => {
        , inlateout("a1") $in => $out
        $crate::syscall!(@genarms @inout 2 $($arms)*),
    };
    (@genarms @inout 2 $in:expr => $out:expr, $($arms:tt)*) => {
        , inlateout("a2") $in => $out
        $crate::syscall!(@genarms @inout 3 $($arms)*),
    };
    (@genarms @inout 3 $in:expr => $out:expr, $($arms:tt)*) => {
        , inlateout("a3") $in => $out
        $crate::syscall!(@genarms @inout 4 $($arms)*),
    };
    (@genarms @inout 4 $in:expr => $out:expr, $($arms:tt)*) => {
        , inlateout("a4") $in => $out
        $crate::syscall!(@genarms @inout 5 $($arms)*),
    };
    (@genarms @inout 5 $in:expr => $out:expr, $($arms:tt)*) => {
        , inlateout("a5") $in => $out
        $crate::syscall!(@genarms @inout 6 $($arms)*),
    };
    (@genarms @inout 6 $in:expr => $out:expr, $($arms:tt)*) => {
        , inlateout("a6") $in => $out
        $crate::syscall!(@genarms @inout 7 $($arms)*),
    };
    (@genarms @inout 7 $in:expr => $out:expr, $($arms:tt)*) => {
        , inlateout("a7") $in => $out
    };


    (@genarms @inout 1 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a1") $out
        $crate::syscall!(@genarms @out 2 $($arms)*),
    };
    (@genarms @out 2 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a2") $out
        $crate::syscall!(@genarms @out 3 $($arms)*),
    };
    (@genarms @inout 2 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a2") $out
        $crate::syscall!(@genarms @out 3 $($arms)*),
    };
    (@genarms @in 2 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a2") $out
        $crate::syscall!(@genarms @out 3 $($arms)*),
    };
    (@genarms @out 3 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a3") $out
        $crate::syscall!(@genarms @out 4 $($arms)*),
    };
    (@genarms @inout 3 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a3") $out
        $crate::syscall!(@genarms @out 4 $($arms)*),
    };
    (@genarms @in 3 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a3") $out
        $crate::syscall!(@genarms @out 4 $($arms)*),
    };
    (@genarms @out 4 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a4") $out
        $crate::syscall!(@genarms @out 5 $($arms)*),
    };
    (@genarms @inout 4 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a4") $out
        $crate::syscall!(@genarms @out 5 $($arms)*),
    };
    (@genarms @in 4 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a4") $out
        $crate::syscall!(@genarms @out 5 $($arms)*),
    };
    (@genarms @out 5 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a5") $out
        $crate::syscall!(@genarms @out 6 $($arms)*),
    };
    (@genarms @inout 5 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a5") $out
        $crate::syscall!(@genarms @out 6 $($arms)*),
    };
    (@genarms @in 5 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a5") $out
        $crate::syscall!(@genarms @out 6 $($arms)*),
    };
    (@genarms @out 6 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a6") $out
        $crate::syscall!(@genarms @out 7 $($arms)*),
    };
    (@genarms @inout 6 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a6") $out
        $crate::syscall!(@genarms @out 7 $($arms)*),
    };
    (@genarms @in 6 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a6") $out
        $crate::syscall!(@genarms @out 7 $($arms)*),
    };
    (@genarms @out 7 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a7") $out
    };

    (@geninouts @out 7 _ => $out:expr, $($arms:tt)*) => {
        , lateout("a7") $out
    };

    (@genarms @inout 1 $in:expr, $($arms:tt)*) => {
        , in("a1") $in
        $crate::syscall!(@genarms @in 2 $($arms)*)
    };
    (@genarms @in 2 $in:expr, $($arms:tt)*) => {
        , in("a2") $in
        $crate::syscall!(@genarms @in 3 $($arms)*)
    };
    (@genarms @in 3 $in:expr, $($arms:tt)*) => {
        , in("a3") $in
        $crate::syscall!(@genarms @in 4 $($arms)*)
    };
    (@genarms @in 4 $in:expr, $($arms:tt)*) => {
        , in("a4") $in
        $crate::syscall!(@genarms @in 5 $($arms)*)
    };
    (@genarms @in 5 $in:expr, $($arms:tt)*) => {
        , in("a5") $in
        $crate::syscall!(@genarms @in 6 $($arms)*)
    };
    (@genarms @in 6 $in:expr, $($arms:tt)*) => {
        , in("a6") $in
        $crate::syscall!(@genarms @in 7 $($arms)*)
    };
    (@genarms @in 7 $in:expr, $($arms:tt)*) => {
        , in("a7") $in
    };

    (@genarms @inout $t:literal) => {};
    (@genarms @in $t:literal) => {};
    (@genarms @out $t:literal) => {};
}
