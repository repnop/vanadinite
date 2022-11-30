// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

macro_rules! impl_dataunit {
    ($($t:ty),+) => {
        $(
            impl DataUnit for $t {
                fn kib(self) -> Bytes {
                    Bytes::new(u64::from(self) * 1024)
                }

                fn mib(self) -> Bytes {
                    Bytes::new(u64::from(self) * 1024 * 1024)
                }

                fn gib(self) -> Bytes {
                    Bytes::new(u64::from(self) * 1024 * 1024 * 1024)
                }

                fn tib(self) -> Bytes {
                    Bytes::new(u64::from(self) * 1024 * 1024 * 1024 * 1024)
                }

                fn kb(self) -> Bytes {
                    Bytes::new(u64::from(self) * 1000)
                }

                fn mb(self) -> Bytes {
                    Bytes::new(u64::from(self) * 1000 * 1000)
                }

                fn gb(self) -> Bytes {
                    Bytes::new(u64::from(self) * 1000 * 1000 * 1000)
                }

                fn tb(self) -> Bytes {
                    Bytes::new(u64::from(self) * 1000 * 1000 * 1000 * 1000)
                }
            }
        )+
    };
}

impl_dataunit!(u8, u16, u32, u64);

pub trait DataUnit {
    fn kib(self) -> Bytes;
    fn mib(self) -> Bytes;
    fn gib(self) -> Bytes;
    fn tib(self) -> Bytes;

    fn kb(self) -> Bytes;
    fn mb(self) -> Bytes;
    fn gb(self) -> Bytes;
    fn tb(self) -> Bytes;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bytes(u64);

impl Bytes {
    pub fn new(bytes: u64) -> Self {
        Self(bytes)
    }

    pub fn get(self) -> u64 {
        self.0
    }

    pub fn to_bits(self) -> Bits {
        Bits::new(self.0 * 8)
    }

    pub fn to_whole_kib(self) -> Kibibytes {
        Kibibytes::new(self.0 / 1u64.kib().get())
    }

    pub fn to_whole_mib(self) -> Mibibytes {
        Mibibytes::new(self.0 / 1u64.mib().get())
    }

    pub fn to_whole_gib(self) -> Gibibytes {
        Gibibytes::new(self.0 / 1u64.gib().get())
    }

    pub fn to_whole_tib(self) -> Tibibytes {
        Tibibytes::new(self.0 / 1u64.tib().get())
    }

    pub fn to_kib(self) -> (Kibibytes, Bytes) {
        (Kibibytes::new(self.0 / 1u64.kib().get()), Self(self.0 % 1u64.kib().get()))
    }

    pub fn to_mib(self) -> (Mibibytes, Bytes) {
        (Mibibytes::new(self.0 / 1u64.mib().get()), Self(self.0 % 1u64.mib().get()))
    }

    pub fn to_gib(self) -> (Gibibytes, Bytes) {
        (Gibibytes::new(self.0 / 1u64.gib().get()), Self(self.0 % 1u64.gib().get()))
    }

    pub fn to_tib(self) -> (Tibibytes, Bytes) {
        (Tibibytes::new(self.0 / 1u64.tib().get()), Self(self.0 % 1u64.tib().get()))
    }
}

impl core::ops::Add for Bytes {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl core::ops::Add<u64> for Bytes {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl core::ops::Add<Bytes> for u64 {
    type Output = Bytes;

    fn add(self, rhs: Bytes) -> Self::Output {
        Bytes(self + rhs.0)
    }
}

impl core::ops::AddAssign<u64> for Bytes {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl core::ops::AddAssign<Bytes> for Bytes {
    fn add_assign(&mut self, rhs: Bytes) {
        self.0 += rhs.0;
    }
}

impl core::ops::Add<u32> for Bytes {
    type Output = Self;

    fn add(self, rhs: u32) -> Self::Output {
        Self(self.0 + u64::from(rhs))
    }
}

impl core::ops::Add<Bytes> for u32 {
    type Output = Bytes;

    fn add(self, rhs: Bytes) -> Self::Output {
        Bytes(u64::from(self) + rhs.0)
    }
}

impl core::ops::AddAssign<u32> for Bytes {
    fn add_assign(&mut self, rhs: u32) {
        self.0 += u64::from(rhs);
    }
}

impl core::ops::Add<u16> for Bytes {
    type Output = Self;

    fn add(self, rhs: u16) -> Self::Output {
        Self(self.0 + u64::from(rhs))
    }
}

impl core::ops::Add<Bytes> for u16 {
    type Output = Bytes;

    fn add(self, rhs: Bytes) -> Self::Output {
        Bytes(u64::from(self) + rhs.0)
    }
}

impl core::ops::AddAssign<u16> for Bytes {
    fn add_assign(&mut self, rhs: u16) {
        self.0 += u64::from(rhs);
    }
}

impl core::ops::Add<u8> for Bytes {
    type Output = Self;

    fn add(self, rhs: u8) -> Self::Output {
        Self(self.0 + u64::from(rhs))
    }
}

impl core::ops::Add<Bytes> for u8 {
    type Output = Bytes;

    fn add(self, rhs: Bytes) -> Self::Output {
        Bytes(u64::from(self) + rhs.0)
    }
}

impl core::ops::AddAssign<u8> for Bytes {
    fn add_assign(&mut self, rhs: u8) {
        self.0 += u64::from(rhs);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Kibibytes(u64);

impl Kibibytes {
    pub fn new(kib: u64) -> Self {
        Self(kib)
    }

    pub fn to_bytes(self) -> Bytes {
        self.0.kib()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mibibytes(u64);

impl Mibibytes {
    pub fn new(mib: u64) -> Self {
        Self(mib)
    }

    pub fn to_bytes(self) -> Bytes {
        self.0.mib()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Gibibytes(u64);

impl Gibibytes {
    pub fn new(gib: u64) -> Self {
        Self(gib)
    }

    pub fn to_bytes(self) -> Bytes {
        self.0.gib()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tibibytes(u64);

impl Tibibytes {
    pub fn new(tib: u64) -> Self {
        Self(tib)
    }

    pub fn to_bytes(self) -> Bytes {
        self.0.tib()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bits(u64);

impl Bits {
    pub fn new(bits: u64) -> Self {
        Self(bits)
    }

    pub fn to_whole_bytes(self) -> Bytes {
        Bytes::new(self.0 / 8)
    }

    pub fn to_bytes(self) -> (Bytes, Bits) {
        (Bytes::new(self.0 / 8), Self::new(self.0 % 8))
    }
}
