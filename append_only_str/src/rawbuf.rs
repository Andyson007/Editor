use std::{
    alloc::{self, Layout},
    ops::RangeBounds,
    ptr::NonNull,
};

use crate::get_range;

pub(crate) struct RawBuf {
    ptr: NonNull<u8>,
    capacity: usize,
}

impl RawBuf {
    pub fn new() -> Self {
        Self {
            ptr: NonNull::dangling(),
            capacity: 0,
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
        Self { ptr, capacity }
    }

    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline(always)]
    pub fn ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for RawBuf {
    fn drop(&mut self) {
        unsafe {
            alloc::dealloc(
                self.ptr.as_ptr(),
                Layout::array::<u8>(self.capacity).unwrap(),
            )
        }
    }
}

/// SAFETY: RawBuf does not allow for interior mutability
/// without exclusive access and is therefore `Sync` & `Send`
unsafe impl Sync for RawBuf {}
unsafe impl Send for RawBuf {}
