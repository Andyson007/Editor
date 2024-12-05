#![feature(never_type)]
#![feature(iter_next_chunk)]
//! This crate implements a custom binary
//! text transfer protocol.
pub mod c2s;
pub mod s2c;

/// Reexports stuff for easier access
pub mod prelude {
    pub use crate::c2s::*;
    pub use crate::s2c::*;
}

use std::{collections::VecDeque, mem};

use utils::{iters::IteratorExt, other::CursorPos};

/// A trait allow for serialization into the Btep™ format
pub trait Serialize {
    /// The method provide by `Serialize`.
    fn serialize(&self) -> VecDeque<u8>;
}

/// `Deserialize` allows for deserialization and is supposed to be the opposite of `Serialize`.
pub trait Deserialize {
    /// The method provided by `Deserialize`
    fn deserialize(data: &[u8]) -> Self;
}

impl Serialize for usize {
    fn serialize(&self) -> VecDeque<u8> {
        (*self as u64).to_be_bytes().into()
    }
}

impl Serialize for char {
    fn serialize(&self) -> VecDeque<u8> {
        (*self as u32).to_be_bytes().into()
    }
}

impl Serialize for CursorPos {
    fn serialize(&self) -> VecDeque<u8> {
        let mut ret = VecDeque::with_capacity(const { mem::size_of::<u64>() * 2 });
        ret.extend(self.row.serialize());
        ret.extend(self.col.serialize());
        ret
    }
}

impl Deserialize for CursorPos {
    fn deserialize(data: &[u8]) -> Self {
        let mut chunks = data
            .iter()
            .copied()
            .chunks::<{ mem::size_of::<u64>() }>()
            .take(2);
        let row = u64::from_be_bytes(chunks.next().unwrap()) as usize;
        let col = u64::from_be_bytes(chunks.next().unwrap()) as usize;
        CursorPos { row, col }
    }
}
