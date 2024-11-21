use std::{
    ops::{Bound, Deref, RangeBounds},
    str,
    sync::Arc,
};

use crate::rawbuf::RawBuf;

/// `ByteSlice` is a slice wrapper valid even through `AppendOnlyStr` reallocations
pub struct ByteSlice {
    pub(crate) raw: Arc<RawBuf>,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl ByteSlice {
    pub fn empty() -> Self {
        Self {
            raw: Arc::new(RawBuf::new()),
            start: 0,
            end: 0,
        }
    }

    /// returns the starting position of the slice
    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    /// returns the ending position of the slice
    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Returns the slice as an actual slice
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        if self.raw.ptr().is_null() {
            return &[];
        }
        //// # SAFETY
        //// We never make the capacity greater than the amount of
        //// space allocated. and therefore a slice won't read
        //// uninitialized memory.
        //// We also know that self.raw isn't a nullptr
        unsafe {
            std::ptr::slice_from_raw_parts(
                self.raw.ptr().cast_const().add(self.start),
                self.end - self.start,
            )
            .as_ref()
            .unwrap_unchecked()
        }
    }

    #[must_use]
    pub fn subslice(&self, range: impl RangeBounds<usize>) -> Self {
        let (start, end) = get_range(range, 0, self.len());
        Self {
            raw: Arc::clone(&self.raw),
            start,
            end,
        }
    }
}

impl PartialEq for ByteSlice {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes() && self.start == other.start && self.end == other.end
    }
}

impl Eq for ByteSlice {}

impl Deref for ByteSlice {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { std::slice::from_raw_parts(self.raw.ptr().add(self.start), self.end - self.start) }
    }
}

impl Clone for ByteSlice {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            start: self.start,
            end: self.end,
        }
    }
}

/// `StrSlice` is a string slice wrapper valid even through `AppendOnlyStr` reallocations
#[derive(Clone)]
pub struct StrSlice {
    pub(crate) byteslice: ByteSlice,
}

impl std::fmt::Debug for StrSlice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(str::from_utf8(&self.byteslice))
            .finish()
    }
}

impl PartialEq for StrSlice {
    fn eq(&self, other: &Self) -> bool {
        self.byteslice == other.byteslice
    }
}

impl StrSlice {
    pub fn empty() -> Self {
        Self {
            byteslice: ByteSlice::empty(),
        }
    }
    /// Returns the underlying byte representation of the string
    #[must_use]
    pub const fn as_bytes(&self) -> &ByteSlice {
        &self.byteslice
    }

    /// Returns the starting position in the buffer of this string slice.
    #[must_use]
    pub const fn start(&self) -> usize {
        self.byteslice.start
    }

    /// Returns the end position in the buffer of this string slice.
    /// # Note
    /// This counts the amount of bytes
    #[must_use]
    pub const fn end(&self) -> usize {
        self.byteslice.end
    }

    /// Returns the length of the str as if were utf-8 encoded
    /// You might want to count the iterators length
    /// ```
    /// # use append_only_str::AppendOnlyStr;
    /// # use std::str::FromStr;
    /// # fn main() {
    ///      let mut append_str = AppendOnlyStr::from_str("test").unwrap();
    ///      let slice = append_str.str_slice(..);
    ///      assert_eq!(slice.chars().count(), 4);
    /// # }
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.byteslice.end - self.byteslice.start
    }

    /// Checks if the slice contains anything whatsoever
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[must_use]
    pub fn subslice(&self, range: impl RangeBounds<usize>) -> Self {
        let (start, end) = get_range(range, 0, self.len());
        Self {
            byteslice: ByteSlice {
                raw: Arc::clone(&self.byteslice.raw),
                start,
                end,
            },
        }
    }
}

impl Deref for StrSlice {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        // # Safety
        // This is safe because we checked for utf-8 compilance when creating the struct
        unsafe { str::from_utf8_unchecked(&self.byteslice) }
    }
}

pub(crate) fn get_range(
    range: impl RangeBounds<usize>,
    min_len: usize,
    max_len: usize,
) -> (usize, usize) {
    let start = match range.start_bound() {
        Bound::Included(&v) => v,
        Bound::Excluded(&v) => v + 1,
        Bound::Unbounded => min_len,
    };
    let end = match range.end_bound() {
        Bound::Included(&v) => v + 1,
        Bound::Excluded(&v) => v,
        Bound::Unbounded => max_len,
    };
    assert!(start <= end);
    assert!(end <= max_len);
    (start, end)
}
