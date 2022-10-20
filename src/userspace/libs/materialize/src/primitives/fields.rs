// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{Fields, Primitive};
use crate::{hash::FxHasher, sealed};

macro_rules! fields {
    ($($t:ident),+) => {
        fields!(@gen $($t),+);
    };

    (@gen $($t:ident),+) => {
        impl<'a, $($t: Primitive<'a>,)+> sealed::Sealed for ($($t,)+) {}
        impl<'a, $($t: Primitive<'a>,)+> Fields<'a> for ($($t,)+) {
            const ID: u64 = FxHasher::new().hash(<fields!(@head $($t),+)>::ID).hash(<Self::Next as Fields>::ID).finish();
            type Head = fields!(@head $($t),+);
            type Next = fields!(@tail $($t),+);
        }

        fields!(@skip1 $($t),+);
    };

    (@gen) => {};

    (@skip1 $head:ident) => {};
    (@skip1 $head:ident, $($t:ident),*) => {
        fields!(@gen $($t),*);
    };

    (@head $head:ident) => {
        $head
    };
    (@head $head:ident, $($t:ident),*) => {
        $head
    };

    (@tail $head:ident) => {()};
    (@tail $head:ident, $($t:ident),*) => {
        ($($t,)*)
    };
}

impl sealed::Sealed for () {}
impl<'a> Fields<'a> for () {
    const ID: u64 = FxHasher::new().hash(<() as Primitive>::ID).finish();
    type Head = ();
    type Next = ();

    fn layout() -> core::alloc::Layout {
        core::alloc::Layout::new::<()>()
    }
}

fields!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z);
