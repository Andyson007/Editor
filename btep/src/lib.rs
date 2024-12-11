#![feature(never_type)]
#![feature(iter_next_chunk)]
#![feature(maybe_uninit_uninit_array)]
#![feature(maybe_uninit_array_assume_init)]
//! This crate implements a custom binary
//! text transfer protocol.
pub mod c2s;
pub mod s2c;

/// Reexports stuff for easier access
pub mod prelude {
    pub use crate::c2s::*;
    pub use crate::s2c::*;
}

use std::{collections::VecDeque, io, mem};

use crossterm::style::Color;
use tokio::io::{AsyncRead, AsyncReadExt};
use utils::other::CursorPos;

/// A trait allow for serialization into the Btep™ format
pub trait Serialize {
    /// The method provide by `Serialize`.
    fn serialize(&self) -> VecDeque<u8>;
}

/// `Deserialize` allows for deserialization and is supposed to be the opposite of `Serialize`.
pub trait Deserialize {
    /// The method provided by `Deserialize`
    fn deserialize<T>(data: &mut T) -> impl std::future::Future<Output = io::Result<Self>>
    where
        Self: Sized,
        T: AsyncReadExt + Unpin + Send;
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
    async fn deserialize<T>(data: &mut T) -> io::Result<Self>
    where
        T: AsyncRead + Unpin,
    {
        let mut buf = [0; 8];
        data.read_exact(&mut buf).await?;
        let row = u64::from_be_bytes(buf) as usize;
        data.read_exact(&mut buf).await?;
        let col = u64::from_be_bytes(buf) as usize;
        Ok(Self { row, col })
    }
}

impl Serialize for Color {
    fn serialize(&self) -> VecDeque<u8> {
        unsafe { mem::transmute::<Color, [u8; mem::size_of::<Color>()]>(*self).into() }
    }
}

impl Deserialize for Color {
    async fn deserialize<T>(data: &mut T) -> io::Result<Self>
    where
        Self: Sized,
        T: AsyncReadExt + Unpin + Send,
    {
        let mut buf = [0; mem::size_of::<Color>()];
        data.read_exact(&mut buf).await?;
        Ok(
            unsafe {
                mem::transmute::<[u8; mem::size_of::<Color>()], crossterm::style::Color>(buf)
            },
        )
    }
}

impl<T> Serialize for Vec<T>
where
    T: Serialize,
{
    fn serialize(&self) -> VecDeque<u8> {
        let mut ret = VecDeque::new();
        ret.extend((self.len() as u64).to_be_bytes());
        for elem in self {
            ret.extend(elem.serialize());
        }
        ret
    }
}

impl<T> Deserialize for Vec<T>
where
    T: Deserialize,
{
    async fn deserialize<R>(data: &mut R) -> io::Result<Self>
    where
        Self: Sized,
        R: AsyncReadExt + Unpin + Send,
    {
        let size = data.read_u64().await? as usize;
        let mut ret = Vec::with_capacity(size);
        for _ in 0..size {
            ret.push(T::deserialize(data).await?);
        }
        Ok(ret)
    }
}

impl<T> Serialize for [T]
where
    T: Serialize,
{
    fn serialize(&self) -> VecDeque<u8> {
        let mut ret = VecDeque::new();
        ret.extend((self.len() as u64).to_be_bytes());
        for elem in self {
            ret.extend(elem.serialize());
        }
        ret
    }
}
