// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::collections::BTreeMap;

use librust::mem::{DmaElement, DmaRegion};
use netstack::MacAddress;
use virtio::{
    devices::net::{
        GsoType, HeaderFlags, LinkStatus, NetDeviceFeatures, NetDeviceFeaturesSplit, VirtIoNetHeaderRx,
        VirtIoNetHeaderTx,
    },
    splitqueue::{DescriptorFlags, SplitVirtqueue, SplitqueueIndex, VirtqueueDescriptor},
    StatusFlag, VirtIoDeviceError,
};

use crate::drivers::DriverError;

const MAX_PACKET_LENGTH: usize = 1500; //65550;

unsafe impl Send for VirtIoNetDevice {}
unsafe impl Sync for VirtIoNetDevice {}

pub struct VirtIoNetDevice {
    device: &'static virtio::devices::net::VirtIoNetDevice,
    receive_queue: SplitVirtqueue,
    transmit_queue: SplitVirtqueue,
    rx_data_buffer: RxDataBuffer,
    rx_buffer_map: BTreeMap<SplitqueueIndex<VirtqueueDescriptor>, usize>,
    tx_data_buffer: TxDataBuffer,
    tx_buffer_map: BTreeMap<SplitqueueIndex<VirtqueueDescriptor>, usize>,
}

impl VirtIoNetDevice {
    pub fn new(device: &'static virtio::devices::net::VirtIoNetDevice) -> Result<Self, VirtIoDeviceError> {
        let mut rx_data_buffer = RxDataBuffer::new(64);
        let mut rx_buffer_map = BTreeMap::new();
        let tx_data_buffer = TxDataBuffer::new(64);
        let tx_buffer_map = BTreeMap::new();
        let mut receive_queue = SplitVirtqueue::new(64).unwrap();
        let transmit_queue = SplitVirtqueue::new(64).unwrap();

        for _ in 0..receive_queue.queue_size() / 2 {
            let descriptor = receive_queue.alloc_descriptor().unwrap();
            let (index, buffer) = rx_data_buffer.alloc().unwrap();

            receive_queue.descriptors.write(
                descriptor,
                VirtqueueDescriptor {
                    address: buffer.physical_address(),
                    length: core::mem::size_of::<VirtIoNetHeaderRx<MAX_PACKET_LENGTH>>() as u32,
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

        device.header.status.set_flag(StatusFlag::DriverOk);

        librust::mem::fence(librust::mem::FenceMode::Write);

        if device.header.status.failed() {
            return Err(VirtIoDeviceError::DeviceError);
        }

        device.header.queue_notify.notify(0);

        Ok(Self { device, receive_queue, transmit_queue, rx_data_buffer, rx_buffer_map, tx_data_buffer, tx_buffer_map })
    }

    pub fn mac_address(&self) -> MacAddress {
        MacAddress::new(self.device.mac.read())
    }

    pub fn link_status(&self) -> LinkStatus {
        unsafe { self.device.link_status() }
    }
}

struct TxDataBuffer {
    buffer: DmaRegion<[VirtIoNetHeaderTx<MAX_PACKET_LENGTH>]>,
    free_indices: VecDeque<u16>,
}

impl TxDataBuffer {
    fn new(len: usize) -> Self {
        Self {
            buffer: unsafe { DmaRegion::zeroed_many(len).unwrap().assume_init() },
            free_indices: (0..len as u16).collect(),
        }
    }

    fn alloc(&mut self) -> Option<(usize, DmaElement<'_, VirtIoNetHeaderTx<MAX_PACKET_LENGTH>>)> {
        let index = self.free_indices.pop_front()? as usize;
        Some((index, self.buffer.get(index).unwrap()))
    }

    fn dealloc(&mut self, index: usize) {
        let index = u16::try_from(index).expect("invalid index supplied");
        assert!(!self.free_indices.contains(&index));
        self.free_indices.push_back(index);
    }

    fn get(&mut self, index: usize) -> Option<DmaElement<'_, VirtIoNetHeaderTx<MAX_PACKET_LENGTH>>> {
        self.buffer.get(index)
    }
}

struct RxDataBuffer {
    buffer: DmaRegion<[VirtIoNetHeaderRx<MAX_PACKET_LENGTH>]>,
    free_indices: VecDeque<u16>,
}

impl RxDataBuffer {
    fn new(len: usize) -> Self {
        Self {
            buffer: unsafe { DmaRegion::zeroed_many(len).unwrap().assume_init() },
            free_indices: (0..len as u16).collect(),
        }
    }

    fn alloc(&mut self) -> Option<(usize, DmaElement<'_, VirtIoNetHeaderRx<MAX_PACKET_LENGTH>>)> {
        let index = self.free_indices.pop_front()? as usize;
        Some((index, self.buffer.get(index).unwrap()))
    }

    fn dealloc(&mut self, index: usize) {
        let index = u16::try_from(index).expect("invalid index supplied");
        assert!(!self.free_indices.contains(&index));
        self.free_indices.push_back(index);
    }

    fn get(&mut self, index: usize) -> Option<DmaElement<'_, VirtIoNetHeaderRx<MAX_PACKET_LENGTH>>> {
        self.buffer.get(index)
    }
}

impl super::NetworkDriver for VirtIoNetDevice {
    fn mac(&self) -> netstack::MacAddress {
        self.mac_address()
    }

    fn process_interrupt(&mut self, _: usize) -> Result<Option<&[u8]>, super::DriverError> {
        self.device.header.interrupt_ack.acknowledge_buffer_used();

        if let Some(used) = self.transmit_queue.used.pop() {
            let descr = SplitqueueIndex::new(used.start_index as u16);
            let index = self.tx_buffer_map.remove(&descr).unwrap();
            self.tx_data_buffer.dealloc(index);
        }

        if let Some(used) = self.receive_queue.used.pop() {
            let descr = SplitqueueIndex::new(used.start_index as u16);
            let data_len = self.receive_queue.descriptors.read(descr).length as usize
                - core::mem::size_of::<VirtIoNetHeaderRx<0>>();
            let index = self.rx_buffer_map.remove(&descr).unwrap();
            // Free index so we have it whenever the caller is done with it
            // TODO: need to add it back to the available queue
            self.rx_data_buffer.dealloc(index);

            let buffer = self.rx_data_buffer.get(index).unwrap();
            let buffer = buffer.get();

            return Ok(Some(&buffer.data[..data_len]));
        }

        Ok(None)
    }

    fn tx_raw(&mut self, f: &dyn Fn(&mut [u8]) -> Option<usize>) -> Result<(), super::DriverError> {
        let (index, mut buffer) = self.tx_data_buffer.alloc().unwrap();
        let header = buffer.get_mut();

        let written = f(&mut header.data[..]).ok_or(DriverError::DataTooLong)?;

        header.flags = HeaderFlags::NONE;
        header.gso_size = 0;
        header.gso_type = GsoType::NONE;
        header.header_len = 0;
        header.checksum_offset = 0;
        header.checksum_start = 0;

        let descr = self.transmit_queue.alloc_descriptor().unwrap();
        self.transmit_queue.descriptors.write(
            descr,
            VirtqueueDescriptor {
                address: buffer.physical_address(),
                length: (core::mem::size_of::<VirtIoNetHeaderTx<0>>() + written) as u32,
                flags: DescriptorFlags::NONE,
                next: SplitqueueIndex::new(0),
            },
        );
        self.transmit_queue.available.push(descr);
        self.tx_buffer_map.insert(descr, index);

        librust::mem::fence(librust::mem::FenceMode::Write);

        self.device.header.queue_notify.notify(1);

        Ok(())
    }
}
