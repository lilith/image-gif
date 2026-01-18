//! Traits used in this library
use crate::io::{Result, Write};

/// Writer extension to write little endian data
pub trait WriteBytesExt<T> {
    /// Writes `T` to a bytes stream. Least significant byte first.
    fn write_le(&mut self, n: T) -> Result<()>;
}

impl<W: Write + ?Sized> WriteBytesExt<u8> for W {
    #[inline(always)]
    fn write_le(&mut self, n: u8) -> Result<()> {
        self.write_all(&[n])
    }
}

impl<W: Write + ?Sized> WriteBytesExt<u16> for W {
    #[inline]
    fn write_le(&mut self, n: u16) -> Result<()> {
        self.write_all(&n.to_le_bytes())
    }
}

impl<W: Write + ?Sized> WriteBytesExt<u32> for W {
    #[inline]
    fn write_le(&mut self, n: u32) -> Result<()> {
        self.write_all(&n.to_le_bytes())
    }
}

impl<W: Write + ?Sized> WriteBytesExt<u64> for W {
    #[inline]
    fn write_le(&mut self, n: u64) -> Result<()> {
        self.write_all(&n.to_le_bytes())
    }
}
