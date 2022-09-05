// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[path = "macros.rs"]
mod macros;

use crate::error::Error;

pub trait Hint<Container: ?Sized> {
    fn is_hinted(&self, container: &Container) -> bool;
}

impl<T: PartialEq> Hint<T> for T {
    fn is_hinted(&self, container: &Self) -> bool {
        self == container
    }
}

impl<T: PartialEq> Hint<[T]> for T {
    fn is_hinted(&self, container: &[T]) -> bool {
        container.contains(self)
    }
}

impl<T: PartialEq, const N: usize> Hint<[T; N]> for T {
    fn is_hinted(&self, container: &[T; N]) -> bool {
        container.contains(self)
    }
}

impl<T: PartialOrd> Hint<core::ops::Range<T>> for T {
    fn is_hinted(&self, container: &core::ops::Range<T>) -> bool {
        container.contains(self)
    }
}

impl<T: PartialOrd> Hint<core::ops::RangeInclusive<T>> for T {
    fn is_hinted(&self, container: &core::ops::RangeInclusive<T>) -> bool {
        container.contains(self)
    }
}

impl<T> Hint<fn(&T) -> bool> for T {
    fn is_hinted(&self, f: &fn(&T) -> bool) -> bool {
        f(self)
    }
}

pub struct Choice<I, O, E: Error, P> {
    pub(super) subparsers: P,
    pub(super) _i: core::marker::PhantomData<I>,
    pub(super) _o: core::marker::PhantomData<O>,
    pub(super) _e: core::marker::PhantomData<E>,
}

pub struct HintedChoice<I, O, E: Error, P> {
    pub(super) subparsers: P,
    pub(super) _i: core::marker::PhantomData<I>,
    pub(super) _o: core::marker::PhantomData<O>,
    pub(super) _e: core::marker::PhantomData<E>,
}
