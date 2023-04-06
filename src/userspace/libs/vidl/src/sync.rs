// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use core::ptr::NonNull;
use librust::{
    capabilities::{Capability, CapabilityPtr, CapabilityRights},
    error::SyscallError,
    syscalls::mem::{allocate_shared_memory, MemoryPermissions},
    units::Bytes,
};
use materialize::DeserializeError;

pub struct SharedBuffer {
    cptr: CapabilityPtr,
    memory: NonNull<[u8]>,
}

impl SharedBuffer {
    pub fn new(size: usize) -> Result<Self, SyscallError> {
        let (cptr, memory) = allocate_shared_memory(Bytes(size), MemoryPermissions::READ_WRITE)?;

        Ok(Self {
            cptr,
            // Safety: the kernel never returns a null pointer on success
            memory: unsafe { NonNull::new_unchecked(memory) },
        })
    }

    /// Access the underlying buffer for reading. Note that this method does not
    /// in any way perform synchronization between two processes, it only
    /// performs the necessary logic to get an up-to-date view of the underlying
    /// memory, and synchronizing between when to read and write is performed
    /// externally.
    ///
    /// Importantly, do not store the reference to the buffer for later use as
    /// this is not guaranteed to provide up-to-date values when read. Each time
    /// new data is written to the buffer, a new call to [`SharedBuffer::read`]
    /// must be made.
    pub fn read(&self) -> &[u8] {
        let ptr = self.memory.as_ptr();
        // Note: Since we're dealing with IPC here, we need to make sure that
        // we've:
        //
        // 1. Performed an appropriate fence so that it is up to date with any
        //    writes that happened
        // 2. Pessimise the pointer via a no-op `asm!` block so that the
        //    compiler won't assume that its the same memory its read before.
        //    This is probably overkill, but oh well.
        librust::mem::fence(librust::mem::FenceMode::Write);
        unsafe { core::arch::asm!("/* {} */", in(reg) ptr as *mut u8) };

        unsafe { &*ptr }
    }

    pub fn copy_from_slice(&mut self, data: &[u8]) -> usize {
        let len = usize::min(self.len(), data.len());

        unsafe { core::ptr::copy_nonoverlapping(data.as_ptr(), self.memory.as_mut_ptr(), len) };
        librust::mem::fence(librust::mem::FenceMode::Write);

        len
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.memory.len()
    }

    /// # Safety
    /// You must ensure that the cloned value is not used to access the underlying memory
    // TODO: remove this and use a 2 buffer approach
    pub unsafe fn clone(&self) -> Self {
        Self { cptr: self.cptr, memory: self.memory }
    }
}

unsafe impl Send for SharedBuffer {}
unsafe impl Sync for SharedBuffer {}

impl core::fmt::Debug for SharedBuffer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SharedBuffer").finish_non_exhaustive()
    }
}

impl materialize::Serializable for SharedBuffer {
    type Primitive<'a> = materialize::primitives::Capability;
}

impl<'de> materialize::Deserialize<'de> for SharedBuffer {
    fn deserialize(
        primitive: <Self as materialize::Serializable>::Primitive<'de>,
        capabilities: &[materialize::CapabilityWithDescription],
    ) -> Result<Self, DeserializeError> {
        match capabilities.get(primitive.index) {
            Some(capability) => match capability.description {
                librust::capabilities::CapabilityDescription::Memory { ptr, len, permissions } => {
                    // FIXME: do syscall here to validate memory cap properties?
                    match permissions & MemoryPermissions::READ_WRITE {
                        true => Ok(Self {
                            cptr: capability.capability.cptr,
                            memory: NonNull::new(core::ptr::slice_from_raw_parts_mut(ptr, len))
                                .ok_or(DeserializeError::InvalidCapabilityProperty)?,
                        }),
                        false => Err(DeserializeError::InvalidCapabilityProperty),
                    }
                }
                _ => Err(DeserializeError::MismatchedCapabilityType),
            },
            None => Err(DeserializeError::NotEnoughCapabilities),
        }
    }
}

impl materialize::Serialize for SharedBuffer {
    fn serialize<'a>(
        &self,
        serializer: <Self::Primitive<'a> as materialize::serialize::serializers::PrimitiveSerializer<'a>>::Serializer,
    ) -> Result<(), materialize::SerializeError> {
        serializer.serialize_capability(Capability {
            cptr: self.cptr,
            rights: CapabilityRights::READ | CapabilityRights::WRITE,
        })
    }
}
