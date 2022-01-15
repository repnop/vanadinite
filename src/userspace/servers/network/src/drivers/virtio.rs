// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::BTreeMap;

use librust::mem::{DmaElement, DmaRegion};
use virtio::{
    devices::net::{GsoType, HeaderFlags, LinkStatus, NetDeviceFeatures, NetDeviceFeaturesSplit},
    splitqueue::{DescriptorFlags, SplitVirtqueue, SplitqueueIndex, VirtqueueDescriptor},
    StatusFlag, VirtIoDeviceError,
};

type VirtIoNetHeader = virtio::devices::net::VirtIoNetHeader<MAX_PACKET_LENGTH>;

const MAX_PACKET_LENGTH: usize = 1500; //65550;
const HEADER_LENGTH: usize = core::mem::size_of::<virtio::devices::net::VirtIoNetHeader<0>>();

pub struct VirtIoNetDevice {
    device: &'static virtio::devices::net::VirtIoNetDevice,
    receive_queue: SplitVirtqueue,
    transmit_queue: SplitVirtqueue,
    rx_data_buffer: DataBuffer,
    rx_buffer_map: BTreeMap<SplitqueueIndex<VirtqueueDescriptor>, usize>,
    tx_data_buffer: DataBuffer,
    tx_buffer_map: BTreeMap<SplitqueueIndex<VirtqueueDescriptor>, usize>,
}

impl VirtIoNetDevice {
    pub fn new(device: &'static virtio::devices::net::VirtIoNetDevice) -> Result<Self, VirtIoDeviceError> {
        let mut rx_data_buffer = DataBuffer::new(8);
        let mut rx_buffer_map = BTreeMap::new();
        let tx_data_buffer = DataBuffer::new(8);
        let tx_buffer_map = BTreeMap::new();
        let mut receive_queue = SplitVirtqueue::new(8).unwrap();
        let transmit_queue = SplitVirtqueue::new(8).unwrap();

        for _ in 0..4 {
            let descriptor = receive_queue.alloc_descriptor().unwrap();
            let (index, buffer) = rx_data_buffer.alloc().unwrap();
            receive_queue.descriptors.write(
                descriptor,
                VirtqueueDescriptor {
                    address: buffer.physical_address(),
                    length: MAX_PACKET_LENGTH as u32 + HEADER_LENGTH as u32,
                    flags: DescriptorFlags::WRITE,
                    next: SplitqueueIndex::new(0),
                },
            );
            receive_queue.available.push(descriptor);
            rx_buffer_map.insert(descriptor, index);
        }

        device.header.status.reset();

        device.header.status.set_flag(StatusFlag::Acknowledge);
        device.header.status.set_flag(StatusFlag::Driver);
        device.header.device_features_select.write(0);

        let mut available_features = device.header.features() as u64;
        device.header.device_features_select.write(1);
        available_features |= (device.header.features() as u64) << 32;

        let available_features = NetDeviceFeatures::new(available_features);
        let mut selected_features = NetDeviceFeatures::none();

        // We require a valid MAC address
        selected_features |= NetDeviceFeatures::MAC_ADDRESS;
        if !(available_features & NetDeviceFeatures::MAC_ADDRESS) {
            return Err(VirtIoDeviceError::FeaturesNotRecognized);
        }

        // We require checksum offloading (for now)
        // selected_features |= NetDeviceFeatures::CHKSUM_OFFLOAD;
        // if !(available_features & NetDeviceFeatures::CHKSUM_OFFLOAD) {
        //     return Err(VirtIoDeviceError::FeaturesNotRecognized);
        // }

        // We require the status information
        selected_features |= NetDeviceFeatures::STATUS;
        if !(available_features & NetDeviceFeatures::STATUS) {
            return Err(VirtIoDeviceError::FeaturesNotRecognized);
        }

        // We require the max MTU information
        // selected_features |= NetDeviceFeatures::MAX_MTU;
        // if !(available_features & NetDeviceFeatures::MAX_MTU) {
        //     return Err(VirtIoDeviceError::FeaturesNotRecognized);
        // }

        // We require speed and duplex information
        // selected_features |= NetDeviceFeatures::SPEED_DUPLEX;
        // if !(available_features & NetDeviceFeatures::SPEED_DUPLEX) {
        //     return Err(VirtIoDeviceError::FeaturesNotRecognized);
        // }

        let NetDeviceFeaturesSplit { low, high } = selected_features.split();
        device.header.driver_features_select.write(0);
        device.header.driver_features.write(low);
        device.header.driver_features_select.write(1);
        device.header.driver_features.write(high);

        device.header.status.set_flag(StatusFlag::FeaturesOk);

        if !device.header.status.is_set(StatusFlag::FeaturesOk) {
            return Err(VirtIoDeviceError::FeaturesNotRecognized);
        }

        // Receive Queue
        device.header.queue_select.write(0);
        librust::mem::fence(librust::mem::FenceMode::Write);
        assert!(device.header.queue_size_max.read() > 0);
        device.header.queue_size.write(receive_queue.queue_size());
        device.header.queue_available.set(receive_queue.available.physical_address());
        device.header.queue_descriptor.set(receive_queue.descriptors.physical_address());
        device.header.queue_used.set(receive_queue.used.physical_address());
        device.header.queue_ready.ready();

        librust::mem::fence(librust::mem::FenceMode::Write);

        println!("Pre tx queue");

        // Transmit Queue
        device.header.queue_select.write(1);
        librust::mem::fence(librust::mem::FenceMode::Write);
        assert!(device.header.queue_size_max.read() > 0);
        device.header.queue_size.write(transmit_queue.queue_size());
        device.header.queue_available.set(transmit_queue.available.physical_address());
        device.header.queue_descriptor.set(transmit_queue.descriptors.physical_address());
        device.header.queue_used.set(transmit_queue.used.physical_address());
        device.header.queue_ready.ready();

        librust::mem::fence(librust::mem::FenceMode::Write);

        println!("Post tx queue");

        device.header.status.set_flag(StatusFlag::DriverOk);

        librust::mem::fence(librust::mem::FenceMode::Write);

        if device.header.status.failed() {
            return Err(VirtIoDeviceError::DeviceError);
        }

        // device.header.queue_notify.notify(1);

        Ok(Self { device, receive_queue, transmit_queue, rx_data_buffer, rx_buffer_map, tx_data_buffer, tx_buffer_map })
    }

    pub fn send_raw(&mut self, data: &[u8]) {
        let (index, mut buffer) = self.tx_data_buffer.alloc().unwrap();
        let header = buffer.get_mut();

        header.data[..data.len()].copy_from_slice(data);
        header.flags = HeaderFlags::NONE;
        header.gso_size = 0;
        header.gso_type = GsoType::NONE;
        header.header_len = 0;
        header.num_buffers = 0;
        header.checksum_offset = 0;
        header.checksum_start = 0;

        let descr = self.transmit_queue.alloc_descriptor().unwrap();
        self.transmit_queue.descriptors.write(
            descr,
            VirtqueueDescriptor {
                address: buffer.physical_address(),
                length: HEADER_LENGTH as u32 + data.len() as u32,
                flags: DescriptorFlags::NONE,
                next: SplitqueueIndex::new(0),
            },
        );
        self.transmit_queue.available.push(descr);
        self.tx_buffer_map.insert(descr, index);

        librust::mem::fence(librust::mem::FenceMode::Write);

        self.device.header.queue_notify.notify(1);
        println!("notified tx queue");
    }

    pub fn mac_address(&self) -> [u8; 6] {
        self.device.mac.read()
    }

    pub fn link_status(&self) -> LinkStatus {
        unsafe { self.device.link_status() }
    }

    // pub fn max_mtu(&self) -> u16 {
    //     unsafe { self.device.mtu() }
    // }

    pub fn recv(&mut self) {
        if let Some(used) = self.receive_queue.used.pop() {
            let descr = SplitqueueIndex::new(used.start_index as u16);
            let size = self.receive_queue.descriptors.read(descr).length as usize;
            let index = *self.rx_buffer_map.get(&descr).unwrap();
            let buffer = self.rx_data_buffer.get(index).unwrap();

            println!("{:?}", &buffer.get().data[..size]);
        }
    }
}

struct DataBuffer {
    buffer: DmaRegion<[VirtIoNetHeader]>,
    free_indices: VecDeque<u16>,
}

impl DataBuffer {
    fn new(len: usize) -> Self {
        Self {
            buffer: unsafe { DmaRegion::zeroed_many(len).unwrap().assume_init() },
            free_indices: (0..len as u16).collect(),
        }
    }

    fn alloc(&mut self) -> Option<(usize, DmaElement<'_, VirtIoNetHeader>)> {
        let index = self.free_indices.pop_front()? as usize;
        Some((index, self.buffer.get(index).unwrap()))
    }

    fn dealloc(&mut self, index: usize) {
        let index = u16::try_from(index).expect("invalid index supplied");
        assert!(!self.free_indices.contains(&index));
        self.free_indices.push_back(index);
    }

    fn get(&self, index: usize) -> Option<DmaElement<'_, VirtIoNetHeader>> {
        self.buffer.get(index)
    }
}
