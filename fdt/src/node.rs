const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_NOP: u32 = 4;
const FDT_END: u32 = 5;

pub struct MemoryNode {
    pub starting_address: u64,
    pub size: u64,
    pub initial_mapped_area: Option<MappedArea>,
}

pub struct MappedArea {
    pub effective_address: u64,
    pub physical_address: u64,
    pub size: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct FdtProperty {
    len: crate::BigEndianU32,
    name_offset: crate::BigEndianU32,
}

pub(crate) unsafe fn find_node(
    mut ptr: *const crate::BigEndianU32,
    name: &str,
    header: *const crate::FdtHeader,
    strings: *const crate::FdtStrings,
    dbg: &mut dyn core::fmt::Write,
) -> Option<*const u32> {
    let mut parts = name.splitn(2, '/');
    let looking_for = parts.next()?;

    writeln!(dbg, "here1: {} - {:?}", (*ptr).get(), looking_for);

    match (*ptr).get() {
        FDT_BEGIN_NODE => advance_ptr(&mut ptr, 4),
        _ => return None,
    }

    let unit_name = cstr_core::CStr::from_ptr(ptr.cast()).to_str().ok()?;
    writeln!(dbg, "here2");
    writeln!(dbg, "{:#p}", ptr);
    advance_ptr(&mut ptr, unit_name.as_bytes().len() + 1);
    let offset = ptr.cast::<u8>().align_offset(4);
    advance_ptr(&mut ptr, offset);
    writeln!(dbg, "{:#p}", ptr);

    writeln!(dbg, "unit_name = {}", unit_name);
    writeln!(dbg, "{}", unit_name.split('@').next()?);

    let mut unit_name_iter = unit_name.split('@');

    if unit_name_iter.next()? != looking_for {
        return None;
    }

    if unit_name_iter.next().is_none() {
        return Some(ptr.cast());
    }

    writeln!(dbg, "here3");

    while ptr
        < (header
            .cast::<u8>()
            .add(header.make_ref().off_dt_struct.get() as usize)
            .add(header.make_ref().size_dt_struct.get() as usize))
        .cast()
    {
        match (*ptr).get() {
            FDT_PROP => {
                writeln!(dbg, "here4");
                advance_ptr(&mut ptr, 4);
                let prop: FdtProperty = *ptr.cast();
                advance_ptr(&mut ptr, core::mem::size_of::<FdtProperty>());
                advance_ptr(&mut ptr, prop.len.get() as usize);
                let offset = ptr.cast::<u8>().align_offset(4);
                advance_ptr(&mut ptr, offset);

                writeln!(dbg, "here10: {:?} - {:#p}", prop, ptr);
            }
            FDT_BEGIN_NODE => match parts.next() {
                Some(name) => {
                    writeln!(dbg, "here7");
                    if let Some(p) = find_node(ptr, name, header, strings, dbg) {
                        return Some(p);
                    }
                }
                None => return Some(ptr.cast()),
            },
            FDT_END_NODE => {
                writeln!(dbg, "here5");
                advance_ptr(&mut ptr, 4);
            }
            FDT_END => {
                writeln!(dbg, "here6");
                advance_ptr(&mut ptr, 4);
                break;
            }
            n => {
                writeln!(dbg, "here8 = {}", n);
                break;
            }
        }
    }

    None
}

unsafe fn advance_ptr(ptr: &mut *const crate::BigEndianU32, bytes: usize) {
    *ptr = ptr.cast::<u8>().add(bytes).cast();
}
