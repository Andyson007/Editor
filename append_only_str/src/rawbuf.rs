use core::str;
use std::{
    alloc::{self, Layout},
    ptr::{self, NonNull},
};

pub(crate) struct RawBuf {
    ptr: NonNull<u8>,
    capacity: usize,
    len: usize,
}

impl RawBuf {
    pub fn new() -> Self {
        Self {
            ptr: NonNull::dangling(),
            capacity: 0,
            len: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity < isize::MAX as usize);
        let layout = Layout::array::<u8>(capacity).unwrap();
        // This is unchecked because alloc returns a nullptr
        // when failing to alloc

        //// SAFETY: -----------------------------------------
        //// alloc is an effectively safe function to call
        //// -------------------------------------------------
        let unchecked_ptr = unsafe { alloc::alloc(layout) };

        let ptr = match NonNull::new(unchecked_ptr) {
            Some(p) => p,
            None => alloc::handle_alloc_error(layout),
        };
        Self {
            ptr,
            capacity,
            len: 0,
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Guarantees that the buffer will be
    /// able to hold n more bytes
    ///
    /// # Panics
    /// The function panics if the new capacity overflows
    pub fn reserve(&mut self, amount: usize) {
        let target = self.len + amount;
        if target <= self.capacity {
            // We have space for the reservation
            return;
        }
        let mut new_capacity = self.capacity;
        while target > new_capacity {
            match new_capacity.checked_mul(2) {
                Some(x) => new_capacity = x,
                // This ensures that the new buffer is at least as large as
                // it is supposed to be. This probably isn't the best handling,
                // but this is an edgecase anyways
                None => new_capacity = target,
            }
        }

        let new_buffer = Self::with_capacity(new_capacity);
        //// SAFETY: -----------------------------
        //// `new_buffer` is a new buffer and can therefore not overlap
        //// with the old buffer.
        //// The target destination has enough space because `new_capacity`
        //// is strictly bigger than `self.capacity`
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.ptr.as_ptr(),
                new_buffer.ptr.as_ptr(),
                self.capacity,
            );
        }

        *self = new_buffer;
    }

    /// # SAFETY
    /// This assumes that the bytes are utf-8 compliant
    pub unsafe fn push_bytes(&mut self, bytes: &[u8]) {
        self.reserve(bytes.len());
        self::ptr::copy(bytes.as_ptr(), self.ptr.as_ptr(), bytes.len());
    }

    pub fn push_str(&mut self, str: &str) {
        unsafe { self.push_bytes(str.as_bytes()) }
    }

    pub fn get_str(&self) -> &str {
        str::from_utf8(unsafe {
            std::ptr::slice_from_raw_parts(self.ptr.as_ptr().cast_const(), self.len)
                .as_ref()
                .unwrap()
        })
        .unwrap()
    }
}
