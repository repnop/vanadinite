pub const EXTENSION_ID: usize = 0x10;

#[derive(Debug, Clone, Copy)]
pub struct SbiSpecVersion {
    pub major: usize,
    pub minor: usize,
}

pub fn spec_version() -> SbiSpecVersion {
    let value: usize;

    unsafe {
        asm!(
            "ecall",
            inout("a6") 0 => _,
            inout("a7") EXTENSION_ID => _,
            out("a1") value,
        );
    }

    SbiSpecVersion { major: (value >> 24) & 0x7f, minor: value & 0xff_ffff }
}

#[derive(Debug, Clone, Copy)]
pub enum SbiImplId {
    BerkeleyBootLoader,
    OpenSbi,
    Xvisor,
    Kvm,
    RustSbi,
    Diosix,
    Other(usize),
}

impl SbiImplId {
    pub fn into_usize(self) -> usize {
        match self {
            SbiImplId::BerkeleyBootLoader => 0,
            SbiImplId::OpenSbi => 1,
            SbiImplId::Xvisor => 2,
            SbiImplId::Kvm => 3,
            SbiImplId::RustSbi => 4,
            SbiImplId::Diosix => 5,
            SbiImplId::Other(n) => n,
        }
    }

    pub fn from_usize(n: usize) -> Self {
        match n {
            0 => SbiImplId::BerkeleyBootLoader,
            1 => SbiImplId::OpenSbi,
            2 => SbiImplId::Xvisor,
            3 => SbiImplId::Kvm,
            4 => SbiImplId::RustSbi,
            5 => SbiImplId::Diosix,
            n => SbiImplId::Other(n),
        }
    }
}

pub fn impl_id() -> SbiImplId {
    let value: usize;
    unsafe {
        asm!(
            "ecall",
            inout("a6") 1 => _,
            inout("a7") EXTENSION_ID => _,
            out("a1") value,
        );
    }

    SbiImplId::from_usize(value)
}

pub fn impl_version() -> usize {
    let value: usize;
    unsafe {
        asm!(
            "ecall",
            inout("a6") 2 => _,
            inout("a7") EXTENSION_ID => _,
            out("a1") value,
        );
    }

    value
}

#[derive(Clone, Copy)]
pub enum ExtensionAvailability {
    Available(core::num::NonZeroUsize),
    Unavailable,
}

pub fn probe_extension(id: usize) -> ExtensionAvailability {
    let value: usize;
    unsafe {
        asm!(
            "ecall",
            in("a0") id,
            inout("a6") 3 => _,
            inout("a7") EXTENSION_ID => _,
            out("a1") value,
        );
    }

    match value {
        0 => ExtensionAvailability::Unavailable,
        n => ExtensionAvailability::Available(unsafe { core::num::NonZeroUsize::new_unchecked(n) }),
    }
}

pub fn mvendorid() -> usize {
    let value: usize;
    unsafe {
        asm!(
            "ecall",
            inout("a6") 4 => _,
            inout("a7") EXTENSION_ID => _,
            out("a1") value,
        );
    }

    value
}

pub fn marchid() -> usize {
    let value: usize;
    unsafe {
        asm!(
            "ecall",
            inout("a6") 5 => _,
            inout("a7") EXTENSION_ID => _,
            out("a1") value,
        );
    }

    value
}

pub fn mimpid() -> usize {
    let value: usize;
    unsafe {
        asm!(
            "ecall",
            inout("a6") 6 => _,
            inout("a7") EXTENSION_ID => _,
            out("a1") value,
        );
    }

    value
}
