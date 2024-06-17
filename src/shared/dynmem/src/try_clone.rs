// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2026 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::store::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AllocatedResourceError<E> {
    Alloc,
    InnerType(E),
}

impl<E> From<core::alloc::AllocError> for AllocatedResourceError<E> {
    fn from(_: core::alloc::AllocError) -> Self {
        Self::Alloc
    }
}

impl<E: core::fmt::Display + core::fmt::Debug> core::error::Error for AllocatedResourceError<E> {}
impl<E: core::fmt::Display> core::fmt::Display for AllocatedResourceError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Alloc => f.write_str("memory allocation failure"),
            AllocatedResourceError::InnerType(e) => write!(f, "{e}"),
        }
    }
}

pub trait TryClone: Sized {
    type Error: core::error::Error;

    fn try_clone(&self) -> Result<Self, Self::Error>;
}

impl<T: Copy> TryClone for T {
    type Error = !;

    fn try_clone(&self) -> Result<Self, Self::Error> {
        Ok(*self)
    }
}

impl<T: TryClone, S: Store + Clone> TryClone for crate::boxed::Box<T, S> {
    type Error = AllocatedResourceError<T::Error>;

    fn try_clone(&self) -> Result<Self, Self::Error> {
        let store = Self::store(self).clone();
        Ok(crate::boxed::Box::new((**self).try_clone().map_err(AllocatedResourceError::InnerType)?, store)?)
    }
}
