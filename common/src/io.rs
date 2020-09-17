#[derive(Debug, Clone, Copy)]
pub struct Block(pub usize);

impl Block {
    pub fn to_bytes<B: BlockDevice>(self, device: &B, offset: Offset) -> usize {
        self.0 * device.block_size() + offset.0
    }
}

impl core::ops::Add for Block {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Block(self.0 + rhs.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Offset(pub usize);

pub trait BlockDevice {
    type Error: core::fmt::Debug + core::fmt::Display;
    fn block_size(&self) -> usize;
    fn num_blocks(&self) -> Option<usize>;
    fn read(&mut self, start: Block, offset: Offset, buf: &mut [u8]) -> Result<(), Self::Error>;
    fn write(&mut self, start: Block, offset: Offset, buf: &[u8]) -> Result<(), Self::Error>;

    fn read_at(&mut self, byte_pos: usize, buf: &mut [u8]) -> Result<(), Self::Error> {
        let (block, offset) = self.block_and_offset(byte_pos);
        self.read(block, offset, buf)
    }

    fn write_at(&mut self, byte_pos: usize, buf: &[u8]) -> Result<(), Self::Error> {
        let (block, offset) = self.block_and_offset(byte_pos);
        self.write(block, offset, buf)
    }

    fn block_and_offset(&self, byte_pos: usize) -> (Block, Offset) {
        (Block(byte_pos / self.block_size()), Offset(byte_pos % self.block_size()))
    }
}
