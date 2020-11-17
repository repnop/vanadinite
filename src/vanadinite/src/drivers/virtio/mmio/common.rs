// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::utils::volatile::{Read, ReadWrite, Volatile, Write};

pub use registers::StatusFlag;

#[repr(C)]
pub struct VirtIoHeader {
    pub magic: Volatile<u32, Read>,
    pub version: Volatile<u32, Read>,
    pub device_id: Volatile<u32, Read>,
    pub vendor_id: Volatile<u32, Read>,
    pub device_features: registers::DeviceFeatures,
    pub device_features_select: Volatile<u32, Write>,
    _reserved1: [u32; 2],
    pub driver_features: Volatile<u32, Read>,
    pub driver_features_select: Volatile<u32, Write>,
    _reserved2: [u32; 2],
    pub queue_select: Volatile<u32, Write>,
    pub queue_size_max: Volatile<u32, Read>,
    pub queue_size: Volatile<u32, Write>,
    pub queue_ready: registers::QueueReady,
    _reserved3: [u32; 2],
    pub queue_notify: Volatile<u32, Write>,
    _reserved4: [u32; 3],
    pub interrupt_status: registers::InterruptStatus,
    pub interrupt_ack: registers::InterruptAck,
    _reserved5: [u32; 2],
    pub status: registers::Status,
    _reserved6: [u32; 3],
    pub queue_descriptor: registers::QueueDescriptor,
    _reserved7: [u32; 2],
    pub queue_available: registers::QueueAvailable,
    _reserved8: [u32; 2],
    pub queue_used: registers::QueueUsed,
    _reserved9: [u32; 21],
    pub config_generation: Volatile<u32, Read>,
}

impl VirtIoHeader {
    pub fn valid_magic(&self) -> bool {
        self.magic.read() == u32::from_le_bytes(*b"virt")
    }

    pub fn device_type(&self) -> Option<DeviceType> {
        DeviceType::from_u32(self.device_id.read())
    }

    pub fn features(&self) -> u32 {
        self.device_features.device_type_feature_bits()
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum DeviceType {
    Reserved = 0,
    NetworkCard = 1,
    BlockDevice = 2,
    Console = 3,
    EntropySource = 4,
    MemoryBallooningTraditional = 5,
    IoMemory = 6,
    Rpmsg = 7,
    ScsiHost = 8,
    Transport9P = 9,
    Mac80211Wlan = 10,
    RprocSerial = 11,
    VirtIoCaif = 12,
    MemoryBalloon = 13,
    GpuDevice = 16,
    TimerClockDevice = 17,
    InputDevice = 18,
    SocketDevice = 19,
    CryptoDevice = 20,
    SignalDistributionModule = 21,
    PstoreDevice = 22,
    IommuDevice = 23,
    MemoryDevice = 24,
}

impl DeviceType {
    pub fn from_u32(n: u32) -> Option<Self> {
        match n {
            0 => Some(DeviceType::Reserved),
            1 => Some(DeviceType::NetworkCard),
            2 => Some(DeviceType::BlockDevice),
            3 => Some(DeviceType::Console),
            4 => Some(DeviceType::EntropySource),
            5 => Some(DeviceType::MemoryBallooningTraditional),
            6 => Some(DeviceType::IoMemory),
            7 => Some(DeviceType::Rpmsg),
            8 => Some(DeviceType::ScsiHost),
            9 => Some(DeviceType::Transport9P),
            10 => Some(DeviceType::Mac80211Wlan),
            11 => Some(DeviceType::RprocSerial),
            12 => Some(DeviceType::VirtIoCaif),
            13 => Some(DeviceType::MemoryBalloon),
            16 => Some(DeviceType::GpuDevice),
            17 => Some(DeviceType::TimerClockDevice),
            18 => Some(DeviceType::InputDevice),
            19 => Some(DeviceType::SocketDevice),
            20 => Some(DeviceType::CryptoDevice),
            21 => Some(DeviceType::SignalDistributionModule),
            22 => Some(DeviceType::PstoreDevice),
            23 => Some(DeviceType::IommuDevice),
            24 => Some(DeviceType::MemoryDevice),
            _ => None,
        }
    }
}

mod registers {
    use super::*;
    use crate::mem::paging::PhysicalAddress;

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct DeviceFeatures(Volatile<u32, Read>);

    impl DeviceFeatures {
        pub fn device_type_feature_bits(&self) -> u32 {
            self.0.read() & 0xFFFFFF
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct QueueReady(Volatile<u32, ReadWrite>);

    impl QueueReady {
        pub fn ready(&self) {
            self.0.write(1);
        }

        pub fn unready(&self) {
            self.0.write(0);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct InterruptStatus(Volatile<u32, Read>);

    impl InterruptStatus {
        pub fn buffer_was_used(&self) -> bool {
            self.0.read() & 1 == 1
        }

        pub fn config_was_changed(&self) -> bool {
            self.0.read() & 2 == 2
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct InterruptAck(Volatile<u32, Write>);

    impl InterruptAck {
        pub fn acknowledge_buffer_used(&self) {
            self.0.write(1);
        }

        pub fn acknowledge_config_change(&self) {
            self.0.write(2);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct Status(Volatile<u32, ReadWrite>);

    impl Status {
        pub fn reset(&self) {
            self.0.write(0);
        }

        pub fn set_flag(&self, flag: StatusFlag) {
            self.0.write(self.0.read() | flag as u32);
            crate::mem::fence();
        }

        pub fn failed(&self) -> bool {
            let bit = StatusFlag::Failed as u32;
            self.0.read() & bit == bit
        }

        pub fn needs_reset(&self) -> bool {
            let bit = StatusFlag::DeviceNeedsReset as u32;
            self.0.read() & bit == bit
        }

        pub fn is_set(&self, flag: StatusFlag) -> bool {
            self.0.read() & flag as u32 == flag as u32
        }
    }

    #[derive(Debug, Clone, Copy)]
    #[repr(u32)]
    pub enum StatusFlag {
        Acknowledge = 1,
        DeviceNeedsReset = 64,
        Driver = 2,
        DriverOk = 4,
        Failed = 128,
        FeaturesOk = 8,
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct QueueDescriptor(Volatile<[u32; 2], ReadWrite>);

    impl QueueDescriptor {
        pub fn set(&self, addr: PhysicalAddress) {
            let low = (addr.as_usize() & 0xFFFF_FFFF) as u32;
            let high = (addr.as_usize() >> 32) as u32;
            self.0[0].write(low);
            self.0[1].write(high);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct QueueAvailable(Volatile<[u32; 2], ReadWrite>);

    impl QueueAvailable {
        pub fn set(&self, addr: PhysicalAddress) {
            let low = (addr.as_usize() & 0xFFFF_FFFF) as u32;
            let high = (addr.as_usize() >> 32) as u32;
            self.0[0].write(low);
            self.0[1].write(high);
        }
    }

    #[derive(Debug)]
    #[repr(transparent)]
    pub struct QueueUsed(Volatile<[u32; 2], ReadWrite>);

    impl QueueUsed {
        pub fn set(&self, addr: PhysicalAddress) {
            let low = (addr.as_usize() & 0xFFFF_FFFF) as u32;
            let high = (addr.as_usize() >> 32) as u32;
            self.0[0].write(low);
            self.0[1].write(high);
        }
    }
}
