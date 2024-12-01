//! Implements iterator helper functions for `AppendOnlyStr`

use crate::{AppendOnlyStr, StrSlice};
use std::str::{self, FromStr};

/// An iterator over the chars inside of the string slices while owning the memory.
pub struct Chars {
    string: StrSlice,
}

impl Iterator for Chars {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.string.chars().next()?;
        self.string.byteslice.start += ret.len_utf8();
        Some(ret)
    }
}

impl AppendOnlyStr {
    /// Creates an iterator over the chars in this `StrSlice`. This allows for the iterator to
    /// outlive this `StrSlice`
    #[must_use]
    pub fn owned_chars(&self) -> Chars {
        self.str_slice(..).owned_chars()
    }

    /// Iterates over the chars using the build in Chars iterator from the standard library
    pub fn chars(&self) -> str::Chars {
        self.get_str().chars()
    }
}

impl StrSlice {
    /// Creates an iterator over the chars in this `StrSlice`. This allows for the iterator to
    /// outlive this `StrSlice`
    #[must_use]
    pub fn owned_chars(&self) -> Chars {
        Chars {
            string: self.clone(),
        }
    }
}

impl FromIterator<u8> for AppendOnlyStr {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = u8>,
    {
        Self::from_str(
            str::from_utf8(iter.into_iter().collect::<Vec<_>>().as_slice())
                .expect("The u8 stream was not utf-8"),
        )
        .unwrap()
    }
}
