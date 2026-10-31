//! Where a reader's bytes come from: the two `no_std` traits every reader reads through, so one code
//! path reads a file, a network range, a zip entry or a SQLite blob, and the result never depends on
//! how the bytes arrive (24.7).
//!
//! What it does: [`ByteSource`] hands out bytes in order, in pieces of whatever size it likes;
//! [`RangeSource`] reads bytes at any offset of a known length. [`read_exact`] and [`read_exact_at`]
//! fill a buffer from either, however the source splits its reads. Byte slices implement both
//! ([`SliceSource`] for the streamed one).
//!
//! Guarantees:
//!
//! - **The split never matters.** [`read_exact`] and [`read_exact_at`] fill the whole buffer or
//!   return an error; a source that returns one byte per call gives the same bytes as one that
//!   returns them all at once.
//! - **A source that stalls is an error, not a loop.** A read that returns 0 before the buffer is
//!   full is [`SourceError::UnexpectedEnd`]; a source that claims more bytes than the buffer holds is
//!   [`SourceError::Overrun`].
//! - **Offsets never overflow.** Every offset is checked against the source's length before it is
//!   read.
//!
//! Method: the reader interfaces of 24.7,
//! `projects/engines/geometry/architecture/24-file-format-exchange.md`, in the manner of
//! `std::io::Read::read_exact` and positioned reads (`pread`), which loop until the buffer is full
//! (<https://doc.rust-lang.org/std/io/trait.Read.html#method.read_exact>).
//!
//! Cost: one call of the source per piece it returns; no allocation. Recursion: none.

use core::fmt;

/// Why a source could not give the bytes asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceError {
    /// The source ended before the bytes asked for.
    UnexpectedEnd,
    /// The source said it wrote more bytes than the buffer holds.
    Overrun,
    /// An offset or length past the source's length.
    OutOfBounds,
    /// The program's source failed, for a reason of its own (a network error, a locked file); the
    /// number is the program's.
    Failed(u32),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::UnexpectedEnd => f.write_str("the source ended early"),
            SourceError::Overrun => f.write_str("the source wrote more than it was asked for"),
            SourceError::OutOfBounds => f.write_str("a read past the end of the source"),
            SourceError::Failed(code) => write!(f, "the source failed ({code})"),
        }
    }
}

impl core::error::Error for SourceError {}

/// Bytes in order, as a file or a network stream gives them (24.7).
pub trait ByteSource {
    /// Reads up to `buf.len()` bytes into the front of `buf`, and says how many; 0 means the source
    /// has ended.
    ///
    /// # Errors
    ///
    /// The source's own failure.
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError>;
}

/// Bytes at any offset of a known length, as a file, a blob or an HTTP range request gives them
/// (24.7).
pub trait RangeSource {
    /// The source's length in bytes.
    fn len(&self) -> u64;

    /// Whether the source holds no bytes.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reads up to `buf.len()` bytes starting at `at` into the front of `buf`, and says how many.
    ///
    /// # Errors
    ///
    /// The source's own failure.
    fn read_at(&self, at: u64, buf: &mut [u8]) -> Result<usize, SourceError>;
}

/// A byte slice read in order.
#[derive(Clone, Copy, Debug)]
pub struct SliceSource<'a> {
    rest: &'a [u8],
}

impl<'a> SliceSource<'a> {
    /// A source of `bytes`, from the first.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes }
    }
}

impl ByteSource for SliceSource<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, SourceError> {
        let n = buf.len().min(self.rest.len());
        let (head, tail) = self.rest.split_at(n);
        buf.get_mut(..n)
            .ok_or(SourceError::Overrun)?
            .copy_from_slice(head);
        self.rest = tail;
        Ok(n)
    }
}

impl RangeSource for [u8] {
    fn len(&self) -> u64 {
        u64::try_from(<[u8]>::len(self)).unwrap_or(u64::MAX)
    }

    fn read_at(&self, at: u64, buf: &mut [u8]) -> Result<usize, SourceError> {
        let at = usize::try_from(at).map_err(|_| SourceError::OutOfBounds)?;
        let rest = self.get(at..).ok_or(SourceError::OutOfBounds)?;
        let n = buf.len().min(rest.len());
        let (Some(into), Some(from)) = (buf.get_mut(..n), rest.get(..n)) else {
            return Err(SourceError::Overrun);
        };
        into.copy_from_slice(from);
        Ok(n)
    }
}

impl<S: RangeSource + ?Sized> RangeSource for &S {
    fn len(&self) -> u64 {
        (**self).len()
    }

    fn read_at(&self, at: u64, buf: &mut [u8]) -> Result<usize, SourceError> {
        (**self).read_at(at, buf)
    }
}

/// Fills `buf` from `source`, however it splits its reads.
///
/// # Errors
///
/// [`SourceError::UnexpectedEnd`] if the source ends first, [`SourceError::Overrun`] if it claims
/// more bytes than were asked for, or the source's own failure.
pub fn read_exact<S: ByteSource + ?Sized>(
    source: &mut S,
    buf: &mut [u8],
) -> Result<(), SourceError> {
    let mut filled = 0;
    while filled < buf.len() {
        let rest = buf.get_mut(filled..).ok_or(SourceError::Overrun)?;
        let want = rest.len();
        let n = source.read(rest)?;
        if n == 0 {
            return Err(SourceError::UnexpectedEnd);
        }
        if n > want {
            return Err(SourceError::Overrun);
        }
        filled = filled.checked_add(n).ok_or(SourceError::Overrun)?;
    }
    Ok(())
}

/// Fills `buf` with the bytes of `source` from offset `at`, however it splits its reads.
///
/// # Errors
///
/// [`SourceError::OutOfBounds`] if the bytes asked for run past the source's length,
/// [`SourceError::UnexpectedEnd`] or [`SourceError::Overrun`] if the source misbehaves, or the
/// source's own failure.
pub fn read_exact_at<S: RangeSource + ?Sized>(
    source: &S,
    at: u64,
    buf: &mut [u8],
) -> Result<(), SourceError> {
    let len = u64::try_from(buf.len()).map_err(|_| SourceError::OutOfBounds)?;
    let end = at.checked_add(len).ok_or(SourceError::OutOfBounds)?;
    if end > source.len() {
        return Err(SourceError::OutOfBounds);
    }
    let mut filled = 0_usize;
    while filled < buf.len() {
        let offset = u64::try_from(filled)
            .ok()
            .and_then(|f| at.checked_add(f))
            .ok_or(SourceError::OutOfBounds)?;
        let rest = buf.get_mut(filled..).ok_or(SourceError::Overrun)?;
        let want = rest.len();
        let n = source.read_at(offset, rest)?;
        if n == 0 {
            return Err(SourceError::UnexpectedEnd);
        }
        if n > want {
            return Err(SourceError::Overrun);
        }
        filled = filled.checked_add(n).ok_or(SourceError::Overrun)?;
    }
    Ok(())
}
