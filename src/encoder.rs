//! # Minimal gif encoder

use alloc::borrow::Cow;
use alloc::fmt;
use alloc::vec::Vec;

use weezl::{encode::Encoder as LzwEncoder, BitOrder};

use crate::common::{AnyExtension, Block, DisposalMethod, Extension, Frame};
use crate::io::{self, Write};
use crate::traits::WriteBytesExt;

/// The image has incorrect properties, making it impossible to encode as a gif.
#[derive(Debug)]
#[non_exhaustive]
pub enum EncodingFormatError {
    /// The image has too many colors.
    TooManyColors,
    /// The image has no color palette which is required.
    MissingColorPalette,
    /// LZW data is not valid for GIF. This may happen when wrong buffer is given to `write_lzw_pre_encoded_frame`
    InvalidMinCodeSize,
}

impl core::error::Error for EncodingFormatError {}
impl fmt::Display for EncodingFormatError {
    #[cold]
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyColors => write!(fmt, "the image has too many colors"),
            Self::MissingColorPalette => write!(
                fmt,
                "the GIF format requires a color palette but none was given"
            ),
            Self::InvalidMinCodeSize => write!(fmt, "LZW data is invalid"),
        }
    }
}

/// Encoding error.
#[derive(Debug)]
#[non_exhaustive]
pub enum EncodingError {
    /// Frame buffer is too small for the declared dimensions.
    FrameBufferTooSmallForDimensions,
    /// Failed to internally allocate a buffer of sufficient size.
    OutOfMemory,
    /// Expected a writer but none found.
    WriterNotFound,
    /// Returned if the to image is not encodable as a gif.
    Format(EncodingFormatError),
    /// Wraps an I/O error.
    Io(io::IoError),
}

impl fmt::Display for EncodingError {
    #[cold]
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameBufferTooSmallForDimensions => {
                fmt.write_str("Frame Buffer Too Small for Dimensions")
            }
            Self::OutOfMemory => fmt.write_str("Out of Memory"),
            Self::WriterNotFound => fmt.write_str("Writer Not Found"),
            Self::Io(err) => err.fmt(fmt),
            Self::Format(err) => err.fmt(fmt),
        }
    }
}

impl core::error::Error for EncodingError {
    #[cold]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::FrameBufferTooSmallForDimensions => None,
            Self::OutOfMemory => None,
            Self::WriterNotFound => None,
            Self::Io(err) => Some(err),
            Self::Format(err) => Some(err),
        }
    }
}

impl From<io::IoError> for EncodingError {
    #[cold]
    fn from(err: io::IoError) -> Self {
        Self::Io(err)
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for EncodingError {
    #[cold]
    fn from(err: std::io::Error) -> Self {
        Self::Io(io::IoError::from(err))
    }
}

impl From<EncodingFormatError> for EncodingError {
    #[cold]
    fn from(err: EncodingFormatError) -> Self {
        Self::Format(err)
    }
}

/// Number of repetitions
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Repeat {
    /// Finite number of repetitions
    Finite(u16),
    /// Infinite number of repetitions
    Infinite,
}

impl Default for Repeat {
    fn default() -> Self {
        Self::Finite(0)
    }
}

/// Extension data.
#[non_exhaustive]
pub enum ExtensionData {
    /// Control extension. Use `ExtensionData::new_control_ext` to construct.
    Control {
        /// Flags.
        flags: u8,
        /// Frame delay.
        delay: u16,
        /// Transparent index.
        trns: u8,
    },
    /// Sets the number of repetitions
    Repetitions(Repeat),
}

impl ExtensionData {
    /// Constructor for control extension data.
    ///
    /// `delay` is given in units of 10 ms.
    #[must_use]
    pub fn new_control_ext(
        delay: u16,
        dispose: DisposalMethod,
        needs_user_input: bool,
        trns: Option<u8>,
    ) -> Self {
        let mut flags = 0;
        let trns = match trns {
            Some(trns) => {
                flags |= 1;
                trns
            }
            None => 0,
        };
        flags |= u8::from(needs_user_input) << 1;
        flags |= (dispose as u8) << 2;
        Self::Control { flags, delay, trns }
    }
}

/// GIF encoder.
pub struct Encoder<W: Write> {
    w: Option<W>,
    global_palette: bool,
    width: u16,
    height: u16,
    buffer: Vec<u8>,
}

impl<W: Write> Encoder<W> {
    /// Creates a new encoder.
    ///
    /// `global_palette` gives the global color palette in the format `[r, g, b, ...]`,
    /// if no global palette shall be used an empty slice may be supplied.
    pub fn new(
        w: W,
        width: u16,
        height: u16,
        global_palette: &[u8],
    ) -> Result<Self, EncodingError> {
        Self {
            w: Some(w),
            global_palette: false,
            width,
            height,
            buffer: Vec::new(),
        }
        .write_global_palette(global_palette)
    }

    /// Write an extension block that signals a repeat behaviour.
    pub fn set_repeat(&mut self, repeat: Repeat) -> Result<(), EncodingError> {
        self.write_extension(ExtensionData::Repetitions(repeat))
    }

    /// Writes the global color palette.
    fn write_global_palette(mut self, palette: &[u8]) -> Result<Self, EncodingError> {
        let mut flags = 0;
        flags |= 0b1000_0000;
        let (palette, padding, table_size) = Self::check_color_table(palette)?;
        self.global_palette = !palette.is_empty();
        flags |= table_size;
        flags |= table_size << 4;
        self.write_screen_desc(flags)?;
        Self::write_color_table(self.writer()?, palette, padding)?;
        Ok(self)
    }

    /// Writes a frame to the image.
    ///
    /// Note: This function also writes a control extension if necessary.
    pub fn write_frame(&mut self, frame: &Frame<'_>) -> Result<(), EncodingError> {
        if usize::from(frame.width)
            .checked_mul(usize::from(frame.height))
            .map_or(true, |size| frame.buffer.len() < size)
        {
            return Err(EncodingError::FrameBufferTooSmallForDimensions);
        }
        debug_assert!(
            (frame.width > 0 && frame.height > 0) || frame.buffer.is_empty(),
            "the frame has 0 pixels, but non-empty buffer"
        );
        self.write_frame_header(frame)?;
        self.write_image_block(&frame.buffer)
    }

    fn write_frame_header(&mut self, frame: &Frame<'_>) -> Result<(), EncodingError> {
        self.write_extension(ExtensionData::new_control_ext(
            frame.delay,
            frame.dispose,
            frame.needs_user_input,
            frame.transparent,
        ))?;
        let mut flags = 0;
        if frame.interlaced {
            flags |= 0b0100_0000;
        }
        let palette = match frame.palette {
            Some(ref palette) => {
                flags |= 0b1000_0000;
                let (palette, padding, table_size) = Self::check_color_table(palette)?;
                flags |= table_size;
                Some((palette, padding))
            }
            None if self.global_palette => None,
            _ => {
                return Err(EncodingError::from(
                    EncodingFormatError::MissingColorPalette,
                ))
            }
        };
        let writer = self.writer()?;
        writer.write_le(Block::Image as u8)?;
        writer.write_le(frame.left)?;
        writer.write_le(frame.top)?;
        writer.write_le(frame.width)?;
        writer.write_le(frame.height)?;
        writer.write_le(flags)?;
        if let Some((palette, padding)) = palette {
            Self::write_color_table(writer, palette, padding)?;
        }
        Ok(())
    }

    fn write_image_block(&mut self, data: &[u8]) -> Result<(), EncodingError> {
        self.buffer.clear();
        self.buffer
            .try_reserve(data.len() / 4)
            .map_err(|_| EncodingError::OutOfMemory)?;
        lzw_encode(data, &mut self.buffer);

        let writer = self.w.as_mut().ok_or(EncodingError::WriterNotFound)?;
        Self::write_encoded_image_block(writer, &self.buffer)
    }

    fn write_encoded_image_block(
        writer: &mut W,
        data_with_min_code_size: &[u8],
    ) -> Result<(), EncodingError> {
        let (&min_code_size, data) = data_with_min_code_size.split_first().unwrap_or((&2, &[]));
        writer.write_le(min_code_size)?;

        let mut iter = data.chunks_exact(0xFF);
        for full_block in iter.by_ref() {
            writer.write_le(0xFFu8)?;
            writer.write_all(full_block)?;
        }
        let last_block = iter.remainder();
        if !last_block.is_empty() {
            writer.write_le(last_block.len() as u8)?;
            writer.write_all(last_block)?;
        }
        writer.write_le(0u8)?;
        Ok(())
    }

    fn write_color_table(
        writer: &mut W,
        table: &[u8],
        padding: usize,
    ) -> Result<(), EncodingError> {
        writer.write_all(table)?;
        for _ in 0..padding {
            writer.write_all(&[0, 0, 0])?;
        }
        Ok(())
    }

    fn check_color_table(table: &[u8]) -> Result<(&[u8], usize, u8), EncodingError> {
        let num_colors = table.len() / 3;
        if num_colors > 256 {
            return Err(EncodingError::from(EncodingFormatError::TooManyColors));
        }
        let table_size = flag_size(num_colors);
        let padding = (2 << table_size) - num_colors;
        Ok((&table[..num_colors * 3], padding, table_size))
    }

    /// Writes an extension to the image.
    ///
    /// It is normally not necessary to call this method manually.
    pub fn write_extension(&mut self, extension: ExtensionData) -> Result<(), EncodingError> {
        use self::ExtensionData::*;
        if let Repetitions(Repeat::Finite(0)) = extension {
            return Ok(());
        }
        let writer = self.writer()?;
        writer.write_le(Block::Extension as u8)?;
        match extension {
            Control { flags, delay, trns } => {
                writer.write_le(Extension::Control as u8)?;
                writer.write_le(4u8)?;
                writer.write_le(flags)?;
                writer.write_le(delay)?;
                writer.write_le(trns)?;
            }
            Repetitions(repeat) => {
                writer.write_le(Extension::Application as u8)?;
                writer.write_le(11u8)?;
                writer.write_all(b"NETSCAPE2.0")?;
                writer.write_le(3u8)?;
                writer.write_le(1u8)?;
                writer.write_le(match repeat {
                    Repeat::Finite(no) => no,
                    Repeat::Infinite => 0u16,
                })?;
            }
        }
        writer.write_le(0u8)?;
        Ok(())
    }

    /// Writes a raw extension to the image.
    ///
    /// This method can be used to write an unsupported extension to the file. `func` is the extension
    /// identifier (e.g. `Extension::Application as u8`). `data` are the extension payload blocks. If any
    /// contained slice has a length > 255 it is automatically divided into sub-blocks.
    pub fn write_raw_extension(
        &mut self,
        func: AnyExtension,
        data: &[&[u8]],
    ) -> Result<(), EncodingError> {
        let writer = self.writer()?;
        writer.write_le(Block::Extension as u8)?;
        writer.write_le(func.0)?;
        for block in data {
            for chunk in block.chunks(0xFF) {
                writer.write_le(chunk.len() as u8)?;
                writer.write_all(chunk)?;
            }
        }
        writer.write_le(0u8)?;
        Ok(())
    }

    /// Writes a frame to the image, but expects `Frame.buffer` to contain LZW-encoded data
    /// from [`Frame::make_lzw_pre_encoded`].
    ///
    /// Note: This function also writes a control extension if necessary.
    pub fn write_lzw_pre_encoded_frame(&mut self, frame: &Frame<'_>) -> Result<(), EncodingError> {
        if let Some(&min_code_size) = frame.buffer.first() {
            if min_code_size > 11 || min_code_size < 2 {
                return Err(EncodingError::Format(
                    EncodingFormatError::InvalidMinCodeSize,
                ));
            }
        }

        self.write_frame_header(frame)?;
        let writer = self.writer()?;
        Self::write_encoded_image_block(writer, &frame.buffer)
    }

    fn write_screen_desc(&mut self, flags: u8) -> Result<(), EncodingError> {
        let width = self.width;
        let height = self.height;
        let writer = self.writer()?;
        writer.write_all(b"GIF89a")?;
        writer.write_le(width)?;
        writer.write_le(height)?;
        writer.write_le(flags)?;
        writer.write_le(0u8)?;
        writer.write_le(0u8)?;
        Ok(())
    }

    /// Gets a reference to the writer instance used by this encoder.
    pub fn get_ref(&self) -> &W {
        self.w.as_ref().unwrap()
    }

    /// Gets a mutable reference to the writer instance used by this encoder.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        self.w.as_mut().unwrap()
    }

    /// Finishes writing, and returns the `io::Write` instance used by this encoder
    pub fn into_inner(mut self) -> Result<W, EncodingError> {
        self.write_trailer()?;
        self.w.take().ok_or(EncodingError::WriterNotFound)
    }

    fn write_trailer(&mut self) -> Result<(), EncodingError> {
        self.writer()?.write_le(Block::Trailer as u8)?;
        Ok(())
    }

    #[inline]
    fn writer(&mut self) -> Result<&mut W, EncodingError> {
        self.w.as_mut().ok_or(EncodingError::WriterNotFound)
    }
}

impl<W: Write> Drop for Encoder<W> {
    #[cfg(feature = "raii_no_panic")]
    fn drop(&mut self) {
        if self.w.is_some() {
            let _ = self.write_trailer();
        }
    }

    #[cfg(not(feature = "raii_no_panic"))]
    fn drop(&mut self) {
        if self.w.is_some() {
            self.write_trailer().unwrap();
        }
    }
}

/// Encodes the data into the provided buffer.
///
/// The first byte is the minimum code size, followed by LZW data.
fn lzw_encode(data: &[u8], buffer: &mut Vec<u8>) {
    let mut max_byte = 0;
    for &byte in data {
        if byte > max_byte {
            max_byte = byte;
            if byte > 127 {
                break;
            }
        }
    }
    let palette_min_len = u32::from(max_byte) + 1;
    let min_code_size = palette_min_len.max(4).next_power_of_two().trailing_zeros() as u8;
    buffer.push(min_code_size);
    let mut enc = LzwEncoder::new(BitOrder::Lsb, min_code_size);
    let len = enc.into_vec(buffer).encode_all(data).consumed_out;
    buffer.truncate(len + 1);
}

impl Frame<'_> {
    /// Replace frame's buffer with a LZW-compressed one for use with [`Encoder::write_lzw_pre_encoded_frame`].
    ///
    /// Frames can be compressed in any order, separately from the `Encoder`, which can be used to compress frames in parallel.
    pub fn make_lzw_pre_encoded(&mut self) {
        let mut buffer = Vec::new();
        buffer.try_reserve(self.buffer.len() / 2).expect("OOM");
        lzw_encode(&self.buffer, &mut buffer);
        self.buffer = Cow::Owned(buffer);
    }
}

// Color table size converted to flag bits
fn flag_size(size: usize) -> u8 {
    (size.clamp(2, 255).next_power_of_two().trailing_zeros() - 1) as u8
}

#[test]
fn test_flag_size() {
    #[rustfmt::skip]
    fn expected(size: usize) -> u8 {
        match size {
            0  ..=2   => 0,
            3  ..=4   => 1,
            5  ..=8   => 2,
            9  ..=16  => 3,
            17 ..=32  => 4,
            33 ..=64  => 5,
            65 ..=128 => 6,
            129..=256 => 7,
            _ => 7
        }
    }

    for i in 0..300 {
        assert_eq!(flag_size(i), expected(i));
    }
    for i in 4..=255u8 {
        let expected = match flag_size(1 + i as usize) + 1 {
            1 => 2,
            n => n,
        };
        let actual = (u32::from(i) + 1)
            .max(4)
            .next_power_of_two()
            .trailing_zeros() as u8;
        assert_eq!(actual, expected);
    }
}

#[test]
fn error_cast() {
    use alloc::boxed::Box;
    let _: Box<dyn core::error::Error> =
        EncodingError::from(EncodingFormatError::MissingColorPalette).into();
}
