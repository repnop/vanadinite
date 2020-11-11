// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod sifive {
    pub mod fu540_c000 {
        pub mod clint;
        pub mod uart;
    }
}

pub mod misc {
    pub mod uart16550;
}

pub trait CompatibleWith {
    fn list() -> &'static [&'static str];
}
