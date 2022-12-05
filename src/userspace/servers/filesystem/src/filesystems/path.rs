// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#[repr(transparent)]
pub struct Path(str);

impl Path {
    pub const fn new(path: &str) -> &Self {
        unsafe { &*(path as *const str as *const Self) }
    }

    pub const fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_absolute(&self) -> bool {
        self.0.starts_with('/')
    }

    pub fn parent(&self) -> Option<&Self> {
        Some(Self::new(self.0.rsplit_once('/')?.0))
    }

    pub fn file_name(&self) -> Option<&str> {
        Some(self.0.rsplit_once('/')?.1)
    }

    pub fn file_extension(&self) -> Option<&str> {
        Some(self.file_name()?.rsplit_once('.')?.1)
    }

    pub fn join(&self, other: &Self) -> PathBuf {
        let mut path = PathBuf::from(self);

        if !path.0.ends_with('/') {
            path.0.push('/');
        }

        match other.0.starts_with('/') {
            true => path.push(&other.0[1..]),
            false => path.push(other),
        }

        path
    }

    pub fn compontents(&self) -> impl Iterator<Item = &'_ str> + '_ {
        self.0.split('/')
    }
}

impl core::ops::Deref for Path {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<Path> for str {
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Self> for Path {
    fn as_ref(&self) -> &Path {
        self
    }
}

pub struct PathBuf(String);

impl PathBuf {
    pub const fn new() -> Self {
        Self(String::new())
    }

    pub fn push<P: AsRef<Path>>(&mut self, path: P) {
        let path = path.as_ref();
        match path.is_absolute() {
            true => *self = Self::from(path),
            false => match self.0.ends_with('/') {
                true => self.0.push_str(path),
                false => {
                    self.0.push('/');
                    self.0.push_str(path);
                }
            },
        }
    }

    pub fn pop(&mut self) -> bool {
        match Path::new(&self.0).parent() {
            Some(parent) => {
                self.0.drain(parent.len()..);
                true
            }
            None => false,
        }
    }
}

impl AsRef<Path> for PathBuf {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

impl core::ops::Deref for PathBuf {
    type Target = Path;
    fn deref(&self) -> &Self::Target {
        Path::new(&self.0)
    }
}

impl From<&'_ Path> for PathBuf {
    fn from(value: &Path) -> Self {
        Self(String::from(&value.0))
    }
}
