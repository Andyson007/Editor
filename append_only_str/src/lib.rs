use std::{
    ptr, str,
    sync::{Arc, RwLock},
};

use rawbuf::RawBuf;
mod rawbuf;

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
    len:usize,
}

impl std::fmt::Debug for AppendOnlyStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppendOnlyStr")
            .field("data", &self.get_str())
            .finish_non_exhaustive()
    }
}

impl Default for AppendOnlyStr {
    fn default() -> Self {
        Self::new()
    }
}

impl AppendOnlyStr {
    pub fn new() -> Self {
        Self {
            rawbuf: Arc::new(RawBuf::new()),
            len: 0.into(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            rawbuf: Arc::new(RawBuf::with_capacity(capacity)),
            len: 0.into(),
        }
    }

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
        let len = *self.len.read().unwrap();
        let target = len + amount;
        if target <= self.rawbuf.capacity() {
            // We have space for the reservation
            return;
        }
        let mut new_capacity = self.rawbuf.capacity();
        while target > new_capacity {
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
        //// `original.rawbuf.capacity()` long
        unsafe {
            std::ptr::copy_nonoverlapping(
                original.rawbuf.ptr(),
                self.rawbuf.ptr(),
                original.rawbuf.capacity(),
            );
        }
    }

    unsafe fn write_unchecked(&mut self, bytes: &[u8]) {
        std::ptr::copy(bytes.as_ptr(), self.rawbuf.ptr(), bytes.len());
        *self.len.write().unwrap() += bytes.len();
    }

    /// # Safety
    /// This assumes that the bytes are utf-8 compliant
    pub unsafe fn push_bytes(&mut self, bytes: &[u8]) {
        self.reserve(bytes.len());
        self.write_unchecked(bytes);
    }

    pub fn push_str(&mut self, str: &str) {
        unsafe { self.push_bytes(str.as_bytes()) }
    }

    pub fn get_str(&self) -> &str {
        //// SAFETY: ---------------------------------------------
        //// We never make the capacity greater than the amount of
        //// space allocated. and therefore a slice won't read
        //// uninitialized memory
        str::from_utf8(unsafe {
            std::ptr::slice_from_raw_parts(
                self.rawbuf.ptr().cast_const(),
                self.len,
            )
            .as_ref()
            .unwrap()
        })
        .unwrap()
    }
}

/// SAFETY: AppendOnlyStr does not allow for interior mutability
/// without exclusive access and is therefore `Sync` & `Send`
unsafe impl Sync for AppendOnlyStr {}
unsafe impl Send for AppendOnlyStr {}
