// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{primitives::Primitive, Message};

// pub struct Deserializer

pub trait Deserialize {
    type Primitive<'a>: Primitive;

    #[inline]
    fn deserialize(primitive: Self::Primitive<'_>) -> Result<Self, ()>;
}

impl Deserialize for u8 {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: Self::Primitive<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}

impl Deserialize for &'_ str {
    type Primitive<'a> = Self;

    #[inline]
    fn deserialize(primitive: Self::Primitive<'_>) -> Result<Self, ()> {
        Ok(primitive)
    }
}
