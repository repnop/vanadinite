use crate::{BigEndianU32, BigEndianU64, Fdt};

const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_NOP: u32 = 4;
const FDT_END: u32 = 5;

#[derive(Debug, Clone, Copy)]
pub struct MemoryNode<'a> {
    pub regions: &'a [MemoryRegion],
    pub initial_mapped_area: Option<MappedArea>,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MemoryRegion {
    starting_address: BigEndianU64,
    size: BigEndianU64,
}

impl MemoryRegion {
    pub fn starting_address(&self) -> *const u8 {
        self.starting_address.get() as usize as *const u8
    }

    pub fn size(&self) -> usize {
        self.size.get() as usize
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MappedArea {
    pub effective_address: u64,
    pub physical_address: u64,
    pub size: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct FdtProperty {
    len: BigEndianU32,
    name_offset: BigEndianU32,
}

pub struct FdtNode<'a> {
    pub name: &'a str,
    header: &'a Fdt,
    props: *const BigEndianU32,
}

impl<'a> FdtNode<'a> {
    fn new(name: &'a str, header: &'a Fdt, props: *const BigEndianU32) -> Self {
        Self { name, header, props }
    }

    pub fn properties(&self) -> impl Iterator<Item = NodeProperty<'_>> {
        let mut ptr = self.props;
        let mut done = false;

        core::iter::from_fn(move || {
            if done {
                return None;
            }

            if unsafe { *ptr }.get() == FDT_PROP {
                Some(NodeProperty::parse(&mut ptr, self.header))
            } else {
                done = true;
                None
            }
        })
    }

    pub fn children(&self) -> impl Iterator<Item = FdtNode<'_>> {
        let mut ptr = self.props;

        while unsafe { *ptr }.get() == FDT_PROP {
            NodeProperty::parse(&mut ptr, self.header);
        }

        let mut done = false;

        core::iter::from_fn(move || {
            if done {
                return None;
            }

            while unsafe { *ptr }.get() == FDT_NOP {
                unsafe { advance_ptr(&mut ptr, 4) };
            }

            if unsafe { *ptr }.get() == FDT_BEGIN_NODE {
                let origin = ptr;
                let ret = unsafe {
                    advance_ptr(&mut ptr, 4);
                    let unit_name = cstr_core::CStr::from_ptr(ptr.cast()).to_str().expect("bad utf8");
                    advance_ptr(&mut ptr, unit_name.as_bytes().len() + 1);
                    let offset = ptr.cast::<u8>().align_offset(4);
                    advance_ptr(&mut ptr, offset);

                    Some(Self::new(unit_name.split('@').next().unwrap(), self.header, ptr))
                };

                ptr = origin;

                unsafe { skip_current_node(&mut ptr, self.header) };

                ret
            } else {
                done = true;
                None
            }
        })
    }

    /// Helper method for finding a `reg` property
    pub fn reg(&self) -> Option<crate::MemoryRegion> {
        let mut reg = None;
        for prop in self.properties() {
            if prop.name == "reg" {
                let mut stream = common::byteorder::IntegerStream::new(prop.value);
                let starting_address = stream.next().expect("starting address");
                let size = stream.next().expect("size");

                reg = Some(MemoryRegion { starting_address, size })
            }
        }

        reg
    }

    /// Helper method for finding a `compatible` property
    pub fn compatible(&self) -> Option<&str> {
        let mut s = None;
        for prop in self.properties() {
            if prop.name == "compatible" {
                let len = prop.value.len() - 1;
                s = core::str::from_utf8(&prop.value[..len]).ok();
            }
        }

        s
    }
}

pub(crate) unsafe fn find_node<'a, 'b: 'a>(
    ptr: &mut *const BigEndianU32,
    name: &str,
    header: &'b Fdt,
) -> Option<FdtNode<'a>> {
    let mut parts = name.splitn(2, '/');
    let looking_for = parts.next()?;

    log::debug!("looking for: {}", looking_for);

    while (**ptr).get() == FDT_NOP {
        advance_ptr(ptr, 4);
    }

    let node_ptr = *ptr;

    match (**ptr).get() {
        FDT_BEGIN_NODE => advance_ptr(ptr, 4),
        _ => return None,
    }

    let unit_name = cstr_core::CStr::from_ptr(ptr.cast()).to_str().ok()?;

    advance_ptr(ptr, unit_name.as_bytes().len() + 1);
    let offset = ptr.cast::<u8>().align_offset(4);

    advance_ptr(ptr, offset);

    let mut unit_name_iter = unit_name.split('@');

    if unit_name_iter.next()? != looking_for {
        log::debug!("found, but don't want it: {}", unit_name);

        *ptr = node_ptr;
        skip_current_node(ptr, header);

        return None;
    }

    log::debug!("found it: {}", unit_name);

    let next_part = match parts.next() {
        Some(part) => part,
        None => return Some(FdtNode::new(unit_name, header, *ptr)),
    };

    while *ptr < header.structs_limit().cast() {
        log::debug!("next_part: {}", next_part);

        while (**ptr).get() == FDT_PROP {
            let prop = NodeProperty::parse(ptr, header);
            log::debug!("parsed prop: {:?}", prop);
        }

        while (**ptr).get() == FDT_BEGIN_NODE {
            log::debug!("FDT_BEGIN_NODE");
            if let Some(p) = find_node(ptr, next_part, header) {
                return Some(p);
            }
        }

        while (**ptr).get() == FDT_NOP {
            advance_ptr(ptr, 4);
        }

        if (**ptr).get() != FDT_END_NODE {
            return None;
        }

        advance_ptr(ptr, 4);
    }

    None
}

pub(crate) unsafe fn skip_current_node(ptr: &mut *const BigEndianU32, header: &Fdt) {
    assert_eq!((**ptr).get(), FDT_BEGIN_NODE, "bad node");
    advance_ptr(ptr, 4);

    let unit_name = cstr_core::CStr::from_ptr(ptr.cast()).to_str().ok().unwrap();
    advance_ptr(ptr, unit_name.as_bytes().len() + 1);
    let offset = ptr.cast::<u8>().align_offset(4);
    advance_ptr(ptr, offset);

    while (**ptr).get() == FDT_PROP {
        NodeProperty::parse(ptr, header);
    }

    while (**ptr).get() == FDT_BEGIN_NODE {
        skip_current_node(ptr, header);
    }

    while (**ptr).get() == FDT_NOP {
        advance_ptr(ptr, 4);
    }

    assert_eq!((**ptr).get(), FDT_END_NODE, "bad node");
    advance_ptr(ptr, 4);
}

#[derive(Debug)]
pub struct NodeProperty<'a> {
    pub name: &'a str,
    pub value: &'a [u8],
}

impl NodeProperty<'_> {
    fn parse(ptr: &mut *const BigEndianU32, header: &Fdt) -> Self {
        unsafe {
            if (**ptr).get() != FDT_PROP {
                panic!("bad prop");
            }

            advance_ptr(ptr, 4);

            let prop: FdtProperty = *ptr.cast();
            let data = ptr.cast::<u8>().add(core::mem::size_of::<FdtProperty>());
            advance_ptr(ptr, core::mem::size_of::<FdtProperty>() + prop.len.get() as usize);
            let offset = ptr.cast::<u8>().align_offset(4);
            advance_ptr(ptr, offset);

            NodeProperty {
                name: header.str_at_offset(prop.name_offset.get() as usize),
                value: core::slice::from_raw_parts(data, prop.len.get() as usize),
            }
        }
    }

    pub fn reg(&self) -> Option<MemoryRegion> {
        match self.name {
            "reg" => {
                let region: *const MemoryRegion = self.value.as_ptr().cast();
                unsafe { Some(*region) }
            }
            _ => None,
        }
    }
}

pub(crate) unsafe fn node_properties<'a>(
    mut node: *const BigEndianU32,
    header: &'a Fdt,
) -> impl Iterator<Item = NodeProperty<'a>> {
    let mut done = false;

    core::iter::from_fn(move || {
        if done {
            return None;
        }

        loop {
            match (*node).get() {
                FDT_PROP => {
                    advance_ptr(&mut node, 4);
                    let prop: FdtProperty = *node.cast();

                    advance_ptr(&mut node, core::mem::size_of::<FdtProperty>());
                    let prop_data_start = node;
                    let prop_data_len = prop.len.get() as usize;

                    advance_ptr(&mut node, prop_data_len);
                    let offset = node.cast::<u8>().align_offset(4);

                    advance_ptr(&mut node, offset);

                    return Some(NodeProperty {
                        name: header.str_at_offset(prop.name_offset.get() as usize),
                        value: core::slice::from_raw_parts(prop_data_start.cast(), prop_data_len),
                    });
                }
                FDT_NOP => advance_ptr(&mut node, 4),
                // FDT_END_NODE or anything else
                _ => {
                    done = true;
                    return None;
                }
            }
        }
    })
}

pub(crate) unsafe fn advance_ptr<T>(ptr: &mut *const T, bytes: usize) {
    *ptr = ptr.cast::<u8>().add(bytes).cast();
}
