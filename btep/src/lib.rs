#![feature(never_type)]
#![feature(iter_next_chunk)]
//! This crate implements a custom binary
//! text transfer protocol.
pub mod s2c;
pub mod c2s;

/// Reexports stuff for easier access
pub mod prelude {
    pub use crate::s2c::*;
    pub use crate::c2s::*;
}

use std::collections::VecDeque;

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
