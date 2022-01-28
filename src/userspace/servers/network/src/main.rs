// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

mod dhcp_helpers;
mod drivers;
use librust::{capabilities::Capability, message::KernelNotification, syscalls::ReadMessage};
use std::{collections::BTreeMap, ipc::IpcChannel};

use crate::drivers::NetworkDriver;

json::derive! {
    #[derive(Debug, Clone)]
    struct Device {
        name: String,
        compatible: Vec<String>,
        interrupts: Vec<usize>,
    }
}

json::derive! {
    Serialize,
    struct VirtIoDeviceRequest {
        ty: u32,
    }
}

json::derive! {
    Deserialize,
    #[derive(Debug)]
    struct VirtIoDeviceResponse {
        devices: Vec<Device>,
    }
}

pub type Action = Box<dyn for<'a> FnOnce(&mut PortAction, &'a [u8])>;

pub enum PortActionCallResult {
    StillValid,
    ClosePort,
}

pub struct PortAction {
    pub action: Option<Action>,
}

impl PortAction {
    pub fn new(action: Action) -> Self {
        Self { action: Some(action) }
    }

    pub fn run(&mut self, data: &[u8]) -> PortActionCallResult {
        if let Some(action) = self.action.take() {
            action(self, data);
        }

        match &self.action {
            Some(_) => PortActionCallResult::StillValid,
            None => PortActionCallResult::ClosePort,
        }
    }
}

fn main() {
    let mut virtiomgr = IpcChannel::new(std::env::lookup_capability("virtiomgr").unwrap());

    virtiomgr
        .send_bytes(&json::to_bytes(&VirtIoDeviceRequest { ty: virtio::DeviceType::NetworkCard as u32 }), &[])
        .unwrap();

    let (message, capabilities) = virtiomgr.read_with_all_caps().unwrap();
    let response: VirtIoDeviceResponse = json::deserialize(message.as_bytes()).unwrap();

    if response.devices.is_empty() {
        return;
    }

    let (Capability { cptr: mmio_cap, .. }, device) = (capabilities[0], &response.devices[0]);
    let info = librust::syscalls::io::query_mmio_cap(mmio_cap).unwrap();

    let net_device = drivers::virtio::VirtIoNetDevice::new(unsafe {
        &*(info.address() as *const virtio::devices::net::VirtIoNetDevice)
    })
    .unwrap();

    let mut network_devices: BTreeMap<usize, Box<dyn NetworkDriver>> = BTreeMap::new();
    assert_eq!(device.interrupts.len(), 1);
    network_devices.insert(device.interrupts[0], Box::new(net_device));

    let mut ports: BTreeMap<u16, PortAction> = BTreeMap::new();

    let mut dhcp_port: u16 = 68;

    for net_device in network_devices.values_mut() {
        let mac = net_device.mac();
        ports.insert(dhcp_port, dhcp_helpers::dhcp_discover(mac, &mut **net_device));
        dhcp_port += 1;
    }

    loop {
        let id = loop {
            match librust::syscalls::receive_message() {
                ReadMessage::Kernel(KernelNotification::InterruptOccurred(id)) => {
                    break id;
                }
                _ => continue,
            }
        };

        if let Some(net_device) = network_devices.get_mut(&id) {
            if let Ok(Some(packet)) = net_device.process_interrupt(id) {
                // TODO: figure out port type and trigger `PortAction`
                println!("{:?}", packet);
            }
        }

        librust::syscalls::io::complete_interrupt(id).unwrap();
    }
}
