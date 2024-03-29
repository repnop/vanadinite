// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

use alchemy::PackedStruct;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct BigEndianU16(u16);

impl BigEndianU16 {
    /// Creates a new [`BigEndianU16`] from a native-endian [`u16`]
    #[inline(always)]
    pub const fn from_ne(n: u16) -> Self {
        Self(n.to_be())
    }

    /// Converts a [`BigEndianU16`] into a native-endian [`u16`]
    #[inline(always)]
    pub const fn to_ne(self) -> u16 {
        u16::from_be_bytes(self.0.to_ne_bytes())
    }

    /// Return the big-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_be_bytes(self) -> [u8; 2] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for BigEndianU16 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct BigEndianU32(u32);

impl BigEndianU32 {
    /// Creates a new [`BigEndianU32`] from a native-endian [`u32`]
    #[inline(always)]
    pub const fn from_ne(n: u32) -> Self {
        Self(n.to_be())
    }

    /// Converts a [`BigEndianU32`] into a native-endian [`u32`]
    #[inline(always)]
    pub const fn to_ne(self) -> u32 {
        u32::from_be_bytes(self.0.to_ne_bytes())
    }

    /// Return the big-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_be_bytes(self) -> [u8; 4] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for BigEndianU32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct BigEndianU64(u64);

impl BigEndianU64 {
    /// Creates a new [`BigEndianU64`] from a native-endian [`u64`]
    #[inline(always)]
    pub const fn from_ne(n: u64) -> Self {
        Self(n.to_be())
    }

    /// Converts a [`BigEndianU64`] into a native-endian [`u64`]
    #[inline(always)]
    pub const fn to_ne(self) -> u64 {
        u64::from_be_bytes(self.0.to_ne_bytes())
    }

    /// Return the big-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_be_bytes(self) -> [u8; 8] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for BigEndianU64 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct BigEndianUsize(usize);

impl BigEndianUsize {
    /// Creates a new [`BigEndianUsize`] from a native-endian [`usize`]
    #[inline(always)]
    pub const fn from_ne(n: usize) -> Self {
        Self(n.to_be())
    }

    /// Converts a [`BigEndianUsize`] into a native-endian [`usize`]
    #[inline(always)]
    pub const fn to_ne(self) -> usize {
        usize::from_be_bytes(self.0.to_ne_bytes())
    }

    /// Converts a [`BigEndianUsize`] into a native-endian [`usize`]
    #[inline(always)]
    pub const fn to_le(self) -> usize {
        usize::from_be_bytes(self.0.to_ne_bytes())
    }

    /// Return the big-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_be_bytes(self) -> [u8; core::mem::size_of::<usize>()] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for BigEndianUsize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct BigEndianI16(i16);

impl BigEndianI16 {
    /// Creates a new [`BigEndianI16`] from a native-endian [`i16`]
    #[inline(always)]
    pub const fn from_ne(n: i16) -> Self {
        Self(n.to_be())
    }

    /// Converts a [`BigEndianI16`] into a native-endian [`i16`]
    #[inline(always)]
    pub const fn to_ne(self) -> i16 {
        i16::from_be_bytes(self.0.to_ne_bytes())
    }

    /// Return the big-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_be_bytes(self) -> [u8; 2] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for BigEndianI16 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct BigEndianI32(i32);

impl BigEndianI32 {
    /// Creates a new [`BigEndianI32`] from a native-endian [`i32`]
    #[inline(always)]
    pub const fn from_ne(n: i32) -> Self {
        Self(n.to_be())
    }

    /// Converts a [`BigEndianI32`] into a native-endian [`i32`]
    #[inline(always)]
    pub const fn to_ne(self) -> i32 {
        i32::from_be_bytes(self.0.to_ne_bytes())
    }

    /// Return the big-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_be_bytes(self) -> [u8; 4] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for BigEndianI32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct BigEndianI64(i64);

impl BigEndianI64 {
    /// Creates a new [`BigEndianI64`] from a native-endian [`i64`]
    #[inline(always)]
    pub const fn from_ne(n: i64) -> Self {
        Self(n.to_be())
    }

    /// Converts a [`BigEndianI64`] into a native-endian [`i64`]
    #[inline(always)]
    pub const fn to_ne(self) -> i64 {
        i64::from_be_bytes(self.0.to_ne_bytes())
    }

    /// Return the big-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_be_bytes(self) -> [u8; 8] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for BigEndianI64 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct BigEndianIsize(isize);

impl BigEndianIsize {
    /// Creates a new [`BigEndianIsize`] from a native-endian [`usize`]
    #[inline(always)]
    pub const fn from_ne(n: isize) -> Self {
        Self(n.to_be())
    }

    /// Converts a [`BigEndianIsize`] into a native-endian [`isize`]
    #[inline(always)]
    pub const fn to_ne(self) -> isize {
        isize::from_be_bytes(self.0.to_ne_bytes())
    }

    /// Return the big-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_be_bytes(self) -> [u8; core::mem::size_of::<isize>()] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for BigEndianIsize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct LittleEndianU16(u16);

impl LittleEndianU16 {
    /// Creates a new [`LittleEndianU16`] from a native-endian [`u16`]
    #[inline(always)]
    pub const fn from_ne(n: u16) -> Self {
        Self(n.to_le())
    }

    /// Converts a [`LittleEndianU16`] into a native-endian [`u16`]
    #[inline(always)]
    pub const fn to_ne(self) -> u16 {
        u16::from_le_bytes(self.0.to_ne_bytes())
    }

    /// Return the little-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for LittleEndianU16 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct LittleEndianU32(u32);

impl LittleEndianU32 {
    /// Creates a new [`LittleEndianU32`] from a native-endian [`u32`]
    #[inline(always)]
    pub const fn from_ne(n: u32) -> Self {
        Self(n.to_le())
    }

    /// Converts a [`LittleEndianU32`] into a native-endian [`u32`]
    #[inline(always)]
    pub const fn to_ne(self) -> u32 {
        u32::from_le_bytes(self.0.to_ne_bytes())
    }

    /// Return the little-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for LittleEndianU32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct LittleEndianU64(u64);

impl LittleEndianU64 {
    /// Creates a new [`LittleEndianU64`] from a native-endian [`u64`]
    #[inline(always)]
    pub const fn from_ne(n: u64) -> Self {
        Self(n.to_le())
    }

    /// Converts a [`LittleEndianU64`] into a native-endian [`u64`]
    #[inline(always)]
    pub const fn to_ne(self) -> u64 {
        u64::from_le_bytes(self.0.to_ne_bytes())
    }

    /// Return the little-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; 8] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for LittleEndianU64 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct LittleEndianUsize(usize);

impl LittleEndianUsize {
    /// Creates a new [`LittleEndianUsize`] from a native-endian [`usize`]
    #[inline(always)]
    pub const fn from_ne(n: usize) -> Self {
        Self(n.to_le())
    }

    /// Converts a [`LittleEndianUsize`] into a native-endian [`usize`]
    #[inline(always)]
    pub const fn to_ne(self) -> usize {
        usize::from_le_bytes(self.0.to_ne_bytes())
    }

    /// Converts a [`LittleEndianUsize`] into a native-endian [`usize`]
    #[inline(always)]
    pub const fn to_le(self) -> usize {
        usize::from_le_bytes(self.0.to_ne_bytes())
    }

    /// Return the little-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; core::mem::size_of::<usize>()] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for LittleEndianUsize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct LittleEndianI16(i16);

impl LittleEndianI16 {
    /// Creates a new [`LittleEndianI16`] from a native-endian [`i16`]
    #[inline(always)]
    pub const fn from_ne(n: i16) -> Self {
        Self(n.to_le())
    }

    /// Converts a [`LittleEndianI16`] into a native-endian [`i16`]
    #[inline(always)]
    pub const fn to_ne(self) -> i16 {
        i16::from_le_bytes(self.0.to_ne_bytes())
    }

    /// Return the little-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for LittleEndianI16 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct LittleEndianI32(i32);

impl LittleEndianI32 {
    /// Creates a new [`LittleEndianI32`] from a native-endian [`i32`]
    #[inline(always)]
    pub const fn from_ne(n: i32) -> Self {
        Self(n.to_le())
    }

    /// Converts a [`LittleEndianI32`] into a native-endian [`i32`]
    #[inline(always)]
    pub const fn to_ne(self) -> i32 {
        i32::from_le_bytes(self.0.to_ne_bytes())
    }

    /// Return the little-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for LittleEndianI32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct LittleEndianI64(i64);

impl LittleEndianI64 {
    /// Creates a new [`LittleEndianI64`] from a native-endian [`i64`]
    #[inline(always)]
    pub const fn from_ne(n: i64) -> Self {
        Self(n.to_le())
    }

    /// Converts a [`LittleEndianI64`] into a native-endian [`i64`]
    #[inline(always)]
    pub const fn to_ne(self) -> i64 {
        i64::from_le_bytes(self.0.to_ne_bytes())
    }

    /// Return the little-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; 8] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for LittleEndianI64 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, PackedStruct)]
#[repr(transparent)]
pub struct LittleEndianIsize(isize);

impl LittleEndianIsize {
    /// Creates a new [`LittleEndianIsize`] from a native-endian [`usize`]
    #[inline(always)]
    pub const fn from_ne(n: isize) -> Self {
        Self(n.to_le())
    }

    /// Converts a [`LittleEndianIsize`] into a native-endian [`isize`]
    #[inline(always)]
    pub const fn to_ne(self) -> isize {
        isize::from_le_bytes(self.0.to_ne_bytes())
    }

    /// Return the little-endian value as a collection of bytes
    #[inline(always)]
    pub const fn to_le_bytes(self) -> [u8; core::mem::size_of::<isize>()] {
        self.0.to_ne_bytes()
    }
}

impl core::fmt::Debug for LittleEndianIsize {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::write!(f, "{}", self.to_ne())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn does_what_its_supposed_to() {
        let n = BigEndianU16::from_ne(0xFF00);
        assert_eq!(n.0, 0x00FF);
        assert_eq!(n.to_ne(), 0xFF00);
    }
}
