//! Implements iterator helper functions for `AppendOnlyStr`

use crate::AppendOnlyStr;
use std::str::{self, FromStr};

impl AppendOnlyStr {
    /// Returns an iterator over all chars
    /// in the `AppendOnlyStr`. This method
    /// gain new elements when push is called
    /// on the original `AppendOnlyStr`
    pub fn chars(&self) -> str::Chars {
        self.get_str().chars()
    }
}

impl FromIterator<u8> for AppendOnlyStr {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = u8>,
    {
        Self::from_str(str::from_utf8(iter.into_iter().collect::<Vec<_>>().as_slice()).unwrap())
            .unwrap()
    }
}
