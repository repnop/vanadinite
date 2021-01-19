#![no_std]

const MAGIC_HEADER_OFFSET: usize = 257;
const MAGIC_HEADER_LENGTH: usize = 6;

pub struct Archive<'a> {
    data: &'a [u8],
}

impl<'a> Archive<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, TarError> {
        match data.get(MAGIC_HEADER_OFFSET..(MAGIC_HEADER_OFFSET + MAGIC_HEADER_LENGTH)) {
            Some([b'u', b's', b't', b'a', b'r', b'\0']) => Ok(Self { data }),
            _ => Err(TarError::InvalidArchive),
        }
    }

    pub fn file(&self, filename: &str) -> Option<File<'a>> {
        let mut archive_index = 0;

        while let Some(header) = self.data.get(archive_index..).and_then(|s| FileHeader::from_bytes(s)) {
            log::debug!("found file: {:?}", header.file_name);

            let content_start = archive_index + 512;
            let content_end = content_start + header.file_size;
            let content_padding = 512 - (header.file_size % 512);

            if header.file_name == filename {
                return Some(File { metadata: header, contents: self.data.get(content_start..content_end)? });
            } else {
                archive_index = content_end + content_padding;
                log::debug!("{:#X}", archive_index);
            }
        }

        None
    }
}

#[derive(Debug)]
pub struct File<'a> {
    pub metadata: FileHeader<'a>,
    pub contents: &'a [u8],
}

#[derive(Debug)]
pub struct FileHeader<'a> {
    pub file_name: &'a str,
    pub file_mode: usize,
    pub uid: usize,
    pub gid: usize,
    pub file_size: usize,
    pub last_modified: usize,
    pub checksum: usize,
    pub type_flag: TypeFlag,
    pub linked_file_name: &'a str,
    pub user_name: &'a str,
    pub group_name: &'a str,
    pub device_major: usize,
    pub device_minor: usize,
    pub file_name_prefix: &'a str,
}

impl<'a> FileHeader<'a> {
    fn from_bytes(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < 512 {
            return None;
        }

        match bytes[MAGIC_HEADER_OFFSET..][..MAGIC_HEADER_LENGTH] {
            [b'u', b's', b't', b'a', b'r', b'\0'] => {}
            _ => return None,
        }

        let file_name = {
            let nul = bytes[..100].iter().copied().position(|b| b == b'\0').unwrap_or(100);
            from_utf8(&bytes[..nul])?
        };

        let file_mode = from_octal_str(&bytes[100..][..8])?;
        let uid = from_octal_str(&bytes[108..][..8])?;
        let gid = from_octal_str(&bytes[116..][..8])?;
        let file_size = from_octal_str(&bytes[124..][..12])?;
        let last_modified = from_octal_str(&bytes[136..][..12])?;
        let checksum = from_octal_str(&bytes[148..][..8])?;
        let type_flag = TypeFlag::from_u8(bytes[156])?;

        let linked_file_name = {
            let nul = bytes[157..][..100].iter().copied().position(|b| b == b'\0').unwrap_or(100);
            from_utf8(&bytes[157..][..nul])?
        };

        let user_name = {
            let nul = bytes[265..][..32].iter().copied().position(|b| b == b'\0').unwrap_or(32);
            from_utf8(&bytes[265..][..nul])?
        };

        let group_name = {
            let nul = bytes[297..][..32].iter().copied().position(|b| b == b'\0').unwrap_or(32);
            from_utf8(&bytes[297..][..nul])?
        };

        let device_major = from_octal_str(&bytes[329..][..8])?;
        let device_minor = from_octal_str(&bytes[337..][..8])?;

        let file_name_prefix = {
            let nul = bytes[345..][..155].iter().copied().position(|b| b == b'\0').unwrap_or(155);
            from_utf8(&bytes[345..][..nul])?
        };

        Some(Self {
            file_name,
            file_mode,
            uid,
            gid,
            file_size,
            last_modified,
            checksum,
            type_flag,
            linked_file_name,
            user_name,
            group_name,
            device_major,
            device_minor,
            file_name_prefix,
        })
    }
}

fn from_utf8(bytes: &[u8]) -> Option<&str> {
    core::str::from_utf8(bytes).ok()
}

fn from_octal_str(bytes: &[u8]) -> Option<usize> {
    let nul = bytes.iter().copied().position(|b| b == b'\0').unwrap_or(bytes.len());
    Some(usize::from_str_radix(from_utf8(&bytes[..nul])?, 8).ok()?)
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum TypeFlag {
    NormalFile = b'0',
    HardLink = b'1',
    SymbolicLink = b'2',
    CharacterSpecial = b'3',
    BlockSpecial = b'4',
    Directory = b'5',
    NamedPipe = b'6',
    ContiguousFile = b'7',
    GlobalExtendedHeader = b'g',
    NextFileExtendedHeader = b'x',
    // Ignoring vendor specific extensions
}

impl TypeFlag {
    fn from_u8(b: u8) -> Option<Self> {
        match b {
            b'0' => Some(Self::NormalFile),
            b'1' => Some(Self::HardLink),
            b'2' => Some(Self::SymbolicLink),
            b'3' => Some(Self::CharacterSpecial),
            b'4' => Some(Self::BlockSpecial),
            b'5' => Some(Self::Directory),
            b'6' => Some(Self::NamedPipe),
            b'7' => Some(Self::ContiguousFile),
            b'g' => Some(Self::GlobalExtendedHeader),
            b'x' => Some(Self::NextFileExtendedHeader),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TarError {
    InvalidArchive,
    BadOctalString,
}
