//! I/O traits and types for no_std support.
//!
//! This module provides unified I/O traits that work in both std and no_std environments.
//! The traits use a fixed `IoError` type to avoid code duplication in the encoder/decoder.

use core::fmt;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

// Re-export ErrorKind for error construction
pub use embedded_io::ErrorKind;

// ============================================================================
// IoError - unified error type
// ============================================================================

/// I/O error type used by this crate.
///
/// In std mode, this wraps `std::io::Error`. In no_std mode, it contains an `ErrorKind`.
#[derive(Debug)]
pub struct IoError {
    #[cfg(feature = "std")]
    inner: std::io::Error,
    #[cfg(not(feature = "std"))]
    kind: ErrorKind,
}

impl IoError {
    /// Create a new error from an ErrorKind.
    #[cfg(not(feature = "std"))]
    #[inline]
    pub fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    /// Create a new error from an ErrorKind.
    #[cfg(feature = "std")]
    #[inline]
    pub fn new(kind: ErrorKind) -> Self {
        let io_kind = match kind {
            ErrorKind::NotFound => std::io::ErrorKind::NotFound,
            ErrorKind::PermissionDenied => std::io::ErrorKind::PermissionDenied,
            ErrorKind::ConnectionRefused => std::io::ErrorKind::ConnectionRefused,
            ErrorKind::ConnectionReset => std::io::ErrorKind::ConnectionReset,
            ErrorKind::ConnectionAborted => std::io::ErrorKind::ConnectionAborted,
            ErrorKind::NotConnected => std::io::ErrorKind::NotConnected,
            ErrorKind::AddrInUse => std::io::ErrorKind::AddrInUse,
            ErrorKind::AddrNotAvailable => std::io::ErrorKind::AddrNotAvailable,
            ErrorKind::BrokenPipe => std::io::ErrorKind::BrokenPipe,
            ErrorKind::AlreadyExists => std::io::ErrorKind::AlreadyExists,
            ErrorKind::InvalidInput => std::io::ErrorKind::InvalidInput,
            ErrorKind::InvalidData => std::io::ErrorKind::InvalidData,
            ErrorKind::TimedOut => std::io::ErrorKind::TimedOut,
            ErrorKind::Interrupted => std::io::ErrorKind::Interrupted,
            ErrorKind::WriteZero => std::io::ErrorKind::WriteZero,
            ErrorKind::OutOfMemory => std::io::ErrorKind::OutOfMemory,
            _ => std::io::ErrorKind::Other,
        };
        Self {
            inner: std::io::Error::new(io_kind, "io error"),
        }
    }

    /// Returns the error kind.
    #[inline]
    pub fn kind(&self) -> ErrorKind {
        #[cfg(feature = "std")]
        {
            match self.inner.kind() {
                std::io::ErrorKind::NotFound => ErrorKind::NotFound,
                std::io::ErrorKind::PermissionDenied => ErrorKind::PermissionDenied,
                std::io::ErrorKind::ConnectionRefused => ErrorKind::ConnectionRefused,
                std::io::ErrorKind::ConnectionReset => ErrorKind::ConnectionReset,
                std::io::ErrorKind::ConnectionAborted => ErrorKind::ConnectionAborted,
                std::io::ErrorKind::NotConnected => ErrorKind::NotConnected,
                std::io::ErrorKind::AddrInUse => ErrorKind::AddrInUse,
                std::io::ErrorKind::AddrNotAvailable => ErrorKind::AddrNotAvailable,
                std::io::ErrorKind::BrokenPipe => ErrorKind::BrokenPipe,
                std::io::ErrorKind::AlreadyExists => ErrorKind::AlreadyExists,
                std::io::ErrorKind::InvalidInput => ErrorKind::InvalidInput,
                std::io::ErrorKind::InvalidData => ErrorKind::InvalidData,
                std::io::ErrorKind::TimedOut => ErrorKind::TimedOut,
                std::io::ErrorKind::Interrupted => ErrorKind::Interrupted,
                std::io::ErrorKind::WriteZero => ErrorKind::WriteZero,
                std::io::ErrorKind::OutOfMemory => ErrorKind::OutOfMemory,
                std::io::ErrorKind::UnexpectedEof => ErrorKind::Other,
                _ => ErrorKind::Other,
            }
        }
        #[cfg(not(feature = "std"))]
        {
            self.kind
        }
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "std")]
        {
            self.inner.fmt(f)
        }
        #[cfg(not(feature = "std"))]
        {
            write!(f, "I/O error: {:?}", self.kind)
        }
    }
}

impl core::error::Error for IoError {
    #[cfg(feature = "std")]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        self.inner.source()
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for IoError {
    #[inline]
    fn from(err: std::io::Error) -> Self {
        Self { inner: err }
    }
}

#[cfg(feature = "std")]
impl From<IoError> for std::io::Error {
    #[inline]
    fn from(err: IoError) -> Self {
        err.inner
    }
}

impl From<ErrorKind> for IoError {
    #[inline]
    fn from(kind: ErrorKind) -> Self {
        Self::new(kind)
    }
}

impl From<core::convert::Infallible> for IoError {
    #[inline]
    fn from(e: core::convert::Infallible) -> Self {
        match e {}
    }
}

/// Result type for I/O operations.
pub type Result<T> = core::result::Result<T, IoError>;

// ============================================================================
// Unified Read trait
// ============================================================================

/// Read trait for GIF decoding with unified error type.
pub trait Read {
    /// Read bytes into buffer, returning number of bytes read.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    /// Read exact number of bytes or error.
    fn read_exact(&mut self, mut buf: &mut [u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => return Err(IoError::new(ErrorKind::Other)), // UnexpectedEof
                Ok(n) => buf = &mut buf[n..],
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

// ============================================================================
// Unified Write trait
// ============================================================================

/// Write trait for GIF encoding with unified error type.
pub trait Write {
    /// Write bytes from buffer, returning number of bytes written.
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    /// Write all bytes or error.
    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => return Err(IoError::new(ErrorKind::WriteZero)),
                Ok(n) => buf = &buf[n..],
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Flush output.
    fn flush(&mut self) -> Result<()>;
}

// ============================================================================
// Unified BufRead trait
// ============================================================================

/// Buffered read trait for GIF decoding.
pub trait BufRead: Read {
    /// Returns buffered data, reading more if needed.
    fn fill_buf(&mut self) -> Result<&[u8]>;

    /// Mark bytes as consumed.
    fn consume(&mut self, amt: usize);
}

// ============================================================================
// std mode: blanket impls for std::io types
// ============================================================================

#[cfg(feature = "std")]
impl<T: std::io::Read + ?Sized> Read for T {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        std::io::Read::read(self, buf).map_err(IoError::from)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        std::io::Read::read_exact(self, buf).map_err(IoError::from)
    }
}

#[cfg(feature = "std")]
impl<T: std::io::Write + ?Sized> Write for T {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        std::io::Write::write(self, buf).map_err(IoError::from)
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        std::io::Write::write_all(self, buf).map_err(IoError::from)
    }

    #[inline]
    fn flush(&mut self) -> Result<()> {
        std::io::Write::flush(self).map_err(IoError::from)
    }
}

#[cfg(feature = "std")]
impl<T: std::io::BufRead + ?Sized> BufRead for T {
    #[inline]
    fn fill_buf(&mut self) -> Result<&[u8]> {
        std::io::BufRead::fill_buf(self).map_err(IoError::from)
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        std::io::BufRead::consume(self, amt)
    }
}

// Re-export std::io::BufReader when std is enabled
#[cfg(feature = "std")]
pub use std::io::BufReader;

// ============================================================================
// Helper traits for conditional bounds
// ============================================================================

/// Marker trait for types that can be wrapped in BufReader.
///
/// In std mode, this requires `std::io::Read` because `std::io::BufReader` needs it.
/// In no_std mode, this requires our `Read` trait for our custom `BufReader`.
#[cfg(feature = "std")]
pub trait ReadBuf: std::io::Read {}

#[cfg(feature = "std")]
impl<T: std::io::Read> ReadBuf for T {}

/// Marker trait for types that can be wrapped in BufReader.
///
/// In std mode, this requires `std::io::Read` because `std::io::BufReader` needs it.
/// In no_std mode, this requires our `Read` trait for our custom `BufReader`.
#[cfg(not(feature = "std"))]
pub trait ReadBuf: Read {}

#[cfg(not(feature = "std"))]
impl<T: Read> ReadBuf for T {}

// ============================================================================
// no_std mode: impls for embedded-io types and common buffers
// ============================================================================

// Fast impl for byte slices (infallible read)
#[cfg(not(feature = "std"))]
impl Read for &[u8] {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let amt = core::cmp::min(buf.len(), self.len());
        let (a, b) = self.split_at(amt);
        buf[..amt].copy_from_slice(a);
        *self = b;
        Ok(amt)
    }
}

// Fast impl for Vec<u8> (infallible write)
#[cfg(not(feature = "std"))]
impl Write for Vec<u8> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.extend_from_slice(buf);
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

// Fast impl for mutable byte slices (infallible write with bounds check)
#[cfg(not(feature = "std"))]
impl Write for &mut [u8] {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let amt = core::cmp::min(buf.len(), self.len());
        let (a, b) = core::mem::take(self).split_at_mut(amt);
        a.copy_from_slice(&buf[..amt]);
        *self = b;
        Ok(amt)
    }

    #[inline]
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

// BufRead for byte slices
#[cfg(not(feature = "std"))]
impl BufRead for &[u8] {
    #[inline]
    fn fill_buf(&mut self) -> Result<&[u8]> {
        Ok(*self)
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        *self = &self[amt..];
    }
}

// ============================================================================
// no_std mode: BufReader implementation
// ============================================================================

/// A buffered reader wrapper for no_std.
#[cfg(not(feature = "std"))]
pub struct BufReader<R> {
    inner: R,
    buf: Vec<u8>,
    pos: usize,
    cap: usize,
}

#[cfg(not(feature = "std"))]
impl<R> BufReader<R> {
    /// Default buffer capacity.
    const DEFAULT_BUF_SIZE: usize = 8192;

    /// Creates a new buffered reader with default buffer capacity.
    #[inline]
    pub fn new(inner: R) -> Self {
        Self::with_capacity(Self::DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a new buffered reader with the specified buffer capacity.
    #[inline]
    pub fn with_capacity(capacity: usize, inner: R) -> Self {
        Self {
            inner,
            buf: vec![0; capacity],
            pos: 0,
            cap: 0,
        }
    }

    /// Gets a reference to the underlying reader.
    #[inline]
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// Gets a mutable reference to the underlying reader.
    #[inline]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Unwraps this `BufReader`, returning the underlying reader.
    #[inline]
    pub fn into_inner(self) -> R {
        self.inner
    }
}

#[cfg(not(feature = "std"))]
impl<R: Read> Read for BufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // If buffer is empty or exhausted, read more
        if self.pos >= self.cap {
            // If the request is larger than our buffer, bypass buffering
            if buf.len() >= self.buf.len() {
                return self.inner.read(buf);
            }
            self.cap = self.inner.read(&mut self.buf)?;
            self.pos = 0;
        }
        // Copy from buffer
        let amt = core::cmp::min(buf.len(), self.cap - self.pos);
        buf[..amt].copy_from_slice(&self.buf[self.pos..self.pos + amt]);
        self.pos += amt;
        Ok(amt)
    }
}

#[cfg(not(feature = "std"))]
impl<R: Read> BufRead for BufReader<R> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        if self.pos >= self.cap {
            self.cap = self.inner.read(&mut self.buf)?;
            self.pos = 0;
        }
        Ok(&self.buf[self.pos..self.cap])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = core::cmp::min(self.pos + amt, self.cap);
    }
}
