//! This is a thread safe append only string. It allows for references to be kept even while the
//! array has to reallocate
//!
//! # Credit
//! A lot of this code looks like
//! [append-only-bytes](https://docs.rs/append-only-bytes/latest/append_only_bytes/index.html),
//! This is because  it didn't have all the methods that I felt it needed. I did rediscover why the
//! architecture was the way it was, but credit where credits due
use std::{
    convert::Infallible,
    num::NonZeroUsize,
    ops::{Index, RangeBounds},
    slice::SliceIndex,
    str::{self, FromStr},
    sync::Arc,
};

use rawbuf::RawBuf;
use slices::{get_range, ByteSlice, StrSlice};
mod rawbuf;

pub mod iters;
pub mod slices;

/// A thread safe append only string
pub struct AppendOnlyStr {
    // The reason there is an arc here is that
    // it allows for references to be kept alive
    // even though the buffer has been reallocated
    // which would invalidate those references
    rawbuf: Arc<RawBuf>,
    // `len` doesn't need to be an RwLock because
    // the only time len is modified we have exclusive
    // access to it
    len: usize,
}

#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for AppendOnlyStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppendOnlyStr")
            .field("data", &&*self.slice(..))
            .field("len", &self.len)
            .finish()
    }
}

impl Default for AppendOnlyStr {
    fn default() -> Self {
        Self::new()
    }
}

impl FromStr for AppendOnlyStr {
    type Err = Infallible;

    fn from_str(data: &str) -> Result<Self, Self::Err> {
        let mut ret = Self::with_capacity(data.len());
        ret.push_str(data);
        Ok(ret)
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<&str> for AppendOnlyStr {
    fn from(value: &str) -> Self {
        Self::from_str(value).unwrap()
    }
}

impl From<String> for AppendOnlyStr {
    fn from(value: String) -> Self {
        value.as_str().into()
    }
}

impl AppendOnlyStr {
    #[must_use]
    #[allow(missing_docs)]
    pub fn new() -> Self {
        Self {
            rawbuf: Arc::new(RawBuf::new()),
            len: 0,
        }
    }

    #[must_use]
    #[allow(missing_docs)]
    pub fn with_capacity(capacity: usize) -> Self {
        NonZeroUsize::new(capacity).map_or_else(Self::new, |nonzero_capacity| Self {
            rawbuf: Arc::new(RawBuf::with_capacity(nonzero_capacity)),
            len: 0,
        })
    }

    /// Appends a single character to the buffer
    pub fn push(c: char) {
        let mut buf = [0; 4];
        c.encode_utf8(&mut buf);
    }

    /// Guarantees that the buffer will be
    /// able to hold n more bytes
    ///
    /// # Panics
    /// The function panics if the new capacity overflows
    pub fn reserve(&mut self, amount: usize) {
        let len = self.len;
        let target = len + amount;
        if target <= self.rawbuf.capacity() {
            // We have space for the reservation
            return;
        }
        let mut new_capacity = self.rawbuf.capacity();

        if new_capacity == 0 {
            new_capacity = 1;
        }

        while new_capacity < target {
            match new_capacity.checked_mul(2) {
                Some(x) => new_capacity = x,
                // This ensures that the new buffer is at least as large as
                // it is supposed to be. This probably isn't the best handling,
                // but this is an edgecase anyways
                None => new_capacity = target,
            }
        }

        let original = std::mem::replace(self, Self::with_capacity(new_capacity));
        //// SAFETY: -----------------------------
        //// The two buffers are non-overlapping, and are both at least
        //// the length of the original buffer
        unsafe {
            std::ptr::copy_nonoverlapping(original.rawbuf.ptr(), self.rawbuf.ptr(), original.len);
        }
        self.len = original.len;
    }

    /// # Safety
    /// This function assumes that there is already enough space allocated to fit the new bytes
    unsafe fn write_unchecked(&mut self, bytes: &[u8]) {
        std::ptr::copy(bytes.as_ptr(), self.rawbuf.ptr().add(self.len), bytes.len());
        self.len += bytes.len();
    }

    /// pushes a series of bytes onto the `AppendOnlyStr`. You are probably looking for `push_str`
    /// # Safety
    /// This assumes that the bytes are utf-8 compliant
    pub unsafe fn push_bytes(&mut self, bytes: &[u8]) {
        self.reserve(bytes.len());
        self.write_unchecked(bytes);
    }

    /// Pushes a string onto the `AppendOnlyStr`
    pub fn push_str(&mut self, str: &str) {
        unsafe { self.push_bytes(str.as_bytes()) }
    }

    /// Returns the length of this appendonly string
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    fn get_str(&self) -> &str {
        // This shouldn't fail because utf-8
        // compliance is always guaranteed
        str::from_utf8(self.get_byte_slice()).unwrap()
    }

    #[must_use]
    fn get_byte_slice(&self) -> &[u8] {
        //// SAFETY: ---------------------------------------------
        //// We never make the capacity greater than the amount of
        //// space allocated. and therefore a slice won't read
        //// uninitialized memory
        unsafe {
            std::ptr::slice_from_raw_parts(self.rawbuf.ptr().cast_const(), self.len)
                .as_ref()
                .unwrap()
        }
    }

    /// Creates a slice referring to that place in memory. This slice is guaranteed to be valid
    /// after the buffer has been reallocated!
    #[must_use]
    pub fn slice(&self, range: impl RangeBounds<usize>) -> ByteSlice {
        let (start, end) = get_range(range, 0, self.len);
        ByteSlice {
            raw: self.rawbuf.clone(),
            start,
            end,
        }
    }

    /// # Panics
    /// This function panics during debug builds to check for valid utf-8 even though they should
    /// always be valid
    pub fn str_slice(&self, range: impl RangeBounds<usize>) -> StrSlice {
        let byteslice = self.slice(range);
        debug_assert!(str::from_utf8(&byteslice).is_ok());
        StrSlice { byteslice }
    }
}

impl<Idx> Index<Idx> for AppendOnlyStr
where
    Idx: SliceIndex<[u8], Output = [u8]>,
{
    type Output = str;

    fn index(&self, index: Idx) -> &Self::Output {
        let tmp = &self.get_byte_slice()[index];
        str::from_utf8(tmp).unwrap()
    }
}

/// SAFETY: `AppendOnlyStr` does not allow for interior mutability
/// without exclusive access and is therefore `Sync` & `Send`
unsafe impl Sync for AppendOnlyStr {}
unsafe impl Send for AppendOnlyStr {}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::AppendOnlyStr;

    #[test]
    fn slice_through_realloc() {
        let mut val = AppendOnlyStr::from_str("test").unwrap();
        let reference = val.slice(0..=1);
        assert_eq!(&*reference, b"te");
        val.push_str("ing stuff");
        let new_ref = val.slice(..6);
        assert_eq!(&*new_ref, &b"testin"[..6]);
        assert_eq!(&*reference, b"te");
    }

    #[test]
    // The reason this could panic is that an empty string wouldn't need any length to be allocated
    // and could therefore be allocated lazily with zero size. Zero-sized allocs panic
    fn zero_size_alloc() {
        let _ = AppendOnlyStr::from_str("");
    }
}
