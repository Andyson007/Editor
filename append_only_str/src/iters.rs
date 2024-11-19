//! Implements iterator helper functions for `AppendOnlyStr`

use crate::{AppendOnlyStr, StrSlice};
use std::str::{self, FromStr};

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
    pub fn owned_chars(&self) -> Chars {
        Chars {
            string: self.str_slice(..),
        }
    }

    pub fn chars(&self) -> str::Chars {
        self.get_str().chars()
    }
}

impl StrSlice {
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
        Self::from_str(str::from_utf8(iter.into_iter().collect::<Vec<_>>().as_slice()).unwrap())
            .unwrap()
    }
}
