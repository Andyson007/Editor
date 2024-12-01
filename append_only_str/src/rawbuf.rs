use std::{
    alloc::{self, Layout},
    num::NonZeroUsize,
    ptr::NonNull,
};

pub struct RawBuf {
    ptr: NonNull<u8>,
    capacity: usize,
}

impl RawBuf {
    pub const fn new() -> Self {
        Self {
            ptr: NonNull::dangling(),
            capacity: 0,
        }
    }

    pub fn with_capacity(capacity: NonZeroUsize) -> Self {
        let layout =
            Layout::array::<u8>(capacity.get()).expect("The capicity was greater than isize::Max");
        // This is unchecked because alloc returns a nullptr
        // when failing to alloc

        //// # SAFETY:
        //// Layout isn't zero-sized
        let unchecked_ptr = unsafe { alloc::alloc(layout) };

        let Some(ptr) = NonNull::new(unchecked_ptr) else {
            alloc::handle_alloc_error(layout)
        };
        Self {
            ptr,
            capacity: capacity.get(),
        }
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    pub const fn ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Drop for RawBuf {
    fn drop(&mut self) {
        if self.capacity != 0 {
            //// # Safety
            //// Dealloc is safe beacuse ptr isn't a nullptr, and we aren't deallocing a zero-sized
            //// struct because we know that we are storing u8s
            unsafe {
                alloc::dealloc(
                    self.ptr.as_ptr(),
                    Layout::array::<u8>(self.capacity).expect(
                        "The capicity exceeded isize::MAX at dealloc. This shouldn't be possible",
                    ),
                );
            }
        }
    }
}

/// SAFETY: `RawBuf` does not allow for interior mutability
/// without exclusive access and is therefore `Sync` & `Send`
unsafe impl Sync for RawBuf {}
unsafe impl Send for RawBuf {}
