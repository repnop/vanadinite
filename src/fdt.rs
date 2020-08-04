use crate::utils::BigEndianU32;

#[repr(C)]
pub struct FdtHeader {
    magic: BigEndianU32,
    totalsize: BigEndianU32,
    off_dt_struct: BigEndianU32,
    off_dt_strings: BigEndianU32,
    off_mem_rsvmap: BigEndianU32,
    version: BigEndianU32,
    last_comp_version: BigEndianU32,
    boot_cpuid_phys: BigEndianU32,
    size_dt_strings: BigEndianU32,
    size_dt_struct: BigEndianU32,
}

impl FdtHeader {
    pub unsafe fn new<'a>(ptr: *const u8) -> Option<&'a Self> {
        assert_eq!(ptr as usize % 4, 0);

        let this: &Self = &*ptr.cast();

        match this.validate_magic() {
            true => Some(this),
            false => None,
        }
    }

    pub fn validate_magic(&self) -> bool {
        self.magic.get() == 0xd00dfeed
    }
}
