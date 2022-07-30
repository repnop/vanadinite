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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(usize)]
pub enum Syscall {
    Exit = 0,
    DebugPrint = 1,
    AllocVirtualMemory = 4,
    GetTid = 5,
    ReadChannel = 7,
    WriteChannel = 9,
    AllocDmaMemory = 12,
    CreateVmspace = 13,
    AllocVmspaceObject = 14,
    SpawnVmspace = 15,
    ClaimDevice = 16,
    QueryMemoryCapability = 20,
    CompleteInterrupt = 21,
    QueryMmioCapability = 22,
    MintCapability = 23,
    RevokeCapability = 24,
    EnableNotifications = 25,
}

impl Syscall {
    pub fn from_usize(n: usize) -> Option<Self> {
        match n {
            0 => Some(Self::Exit),
            1 => Some(Self::DebugPrint),
            4 => Some(Self::AllocVirtualMemory),
            5 => Some(Self::GetTid),
            7 => Some(Self::ReadChannel),
            9 => Some(Self::WriteChannel),
            12 => Some(Self::AllocDmaMemory),
            13 => Some(Self::CreateVmspace),
            14 => Some(Self::AllocVmspaceObject),
            15 => Some(Self::SpawnVmspace),
            16 => Some(Self::ClaimDevice),
            20 => Some(Self::QueryMemoryCapability),
            21 => Some(Self::CompleteInterrupt),
            22 => Some(Self::QueryMmioCapability),
            23 => Some(Self::MintCapability),
            24 => Some(Self::RevokeCapability),
            25 => Some(Self::EnableNotifications),
            _ => None,
        }
    }
}
