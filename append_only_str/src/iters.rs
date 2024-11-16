//! Implements iterator helper functions for `AppendOnlyStr`

use crate::AppendOnlyStr;
use std::str;

impl AppendOnlyStr {
    /// Returns an iterator over all chars
    /// in the `AppendOnlyStr`. This method
    /// gain new elements when push is called
    /// on the original `AppendOnlyStr`
    pub fn chars(&self) -> str::Chars {
        self.get_str().chars()
    }
}
