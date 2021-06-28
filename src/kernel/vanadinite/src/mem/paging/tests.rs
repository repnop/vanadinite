// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::*;
use vanadinite_macros::test;

#[test]
fn virtual_addr_from_vpns() {
    let addr = VirtualAddress::new(0x1F_FF2F_F000);
    assert_eq!(addr, VirtualAddress::from_vpns(addr.vpns()));
}

#[test]
fn virtual_addr_checks() {
    assert!(VirtualAddress::userspace_range().end.checked_add(1).is_none());
    assert!(VirtualAddress::kernelspace_range().start.checked_offset(-1).is_none());
    assert!(VirtualAddress::userspace_range().end.checked_add(0xffffff8000000000).is_none());
    assert!(VirtualAddress::kernelspace_range().start.checked_offset(-1).is_none());
}
