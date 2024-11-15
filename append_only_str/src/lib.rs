use core::fmt;
use std::cell::Cell;

use rawbuf::RawBuf;
mod rawbuf;

pub struct AppendOnlyStr {
    data: RawBuf,
}

impl std::fmt::Debug for AppendOnlyStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppendOnlyStr")
            .field("data", &self.data.get_str())
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
            data: RawBuf::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: RawBuf::with_capacity(capacity),
        }
    }

    pub fn push(c: char) {
        let mut buf = [0; 4];
        c.encode_utf8(&mut buf);
    }

    pub fn push_str(c: &str) {}
}
