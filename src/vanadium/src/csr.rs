// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod misa {

    #[derive(Debug, Clone, Copy)]
    pub struct Misa(usize);

    impl Misa {
        pub fn extensions(self) -> impl Iterator<Item = char> {
            (0..26u8).filter_map(move |n| match (self.0 >> n) & 1 {
                0 => None,
                1 => Some((b'A' + n) as char),
                _ => unreachable!(),
            })
        }
    }

    pub fn read() -> Misa {
        let misa: usize;
        unsafe { asm!("csrr {}, misa", out(reg) misa) };
        Misa(misa)
    }
}

pub mod mvendorid {
    pub fn read() -> usize {
        let mvendorid: usize;
        unsafe { asm!("csrr {}, mvendorid", out(reg) mvendorid) };
        mvendorid
    }
}

pub mod marchid {
    pub fn read() -> usize {
        let marchid: usize;
        unsafe { asm!("csrr {}, marchid", out(reg) marchid) };
        marchid
    }
}

pub mod mimpid {
    pub fn read() -> usize {
        let mimpid: usize;
        unsafe { asm!("csrr {}, mimpid", out(reg) mimpid) };
        mimpid
    }
}

pub mod mhartid {
    pub fn read() -> usize {
        let mhartid: usize;
        unsafe { asm!("csrr {}, mhartid", out(reg) mhartid) };
        mhartid
    }
}
