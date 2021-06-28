// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod sifive {
    pub mod fu540_c000 {
        pub mod uart;
    }
}

pub mod generic {
    pub mod plic;
    pub mod uart16550;
}

pub mod virtio {
    pub mod mmio {
        pub mod block;
        pub mod common;
    }

    pub mod block;
    pub mod queue;

    #[derive(Debug)]
    pub enum VirtIoDeviceError {
        FeaturesNotRecognized,
        DeviceError,
    }
}

pub trait CompatibleWith {
    fn compatible_with() -> &'static [&'static str];
}

pub trait InterruptServicable {
    fn isr(source: usize, private: usize) -> Result<(), &'static str>;
}
