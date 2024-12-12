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

use core::str;
use std::{collections::VecDeque, io, mem};

use crossterm::style::Color;
use tokio::io::{AsyncRead, AsyncReadExt};
use utils::other::CursorPos;

/// A trait allow for serialization into the Btep™ format
pub trait Serialize {
    /// The method provide by `Serialize`.
    fn serialize(&self) -> Vec<u8>;
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
    fn serialize(&self) -> Vec<u8> {
        (*self as u64).to_be_bytes().into()
    }
}

impl Serialize for char {
    fn serialize(&self) -> Vec<u8> {
        (*self as u32).to_be_bytes().into()
    }
}

impl Serialize for CursorPos {
    fn serialize(&self) -> Vec<u8> {
        let mut ret = Vec::with_capacity(const { mem::size_of::<u64>() * 2 });
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
    fn serialize(&self) -> Vec<u8> {
        [match self {
            Self::Reset => 0,
            Self::Black => 1,
            Self::DarkGrey => 2,
            Self::Red => 3,
            Self::DarkRed => 4,
            Self::Green => 5,
            Self::DarkGreen => 6,
            Self::Yellow => 7,
            Self::DarkYellow => 8,
            Self::Blue => 9,
            Self::DarkBlue => 10,
            Self::Magenta => 11,
            Self::DarkMagenta => 12,
            Self::Cyan => 13,
            Self::DarkCyan => 14,
            Self::White => 15,
            Self::Grey => 16,
            Self::Rgb { r, g, b } => return [17, *r, *g, *b].into(),
            Self::AnsiValue(x) => return [18, *x].into(),
        }]
        .into()
    }
}

impl Deserialize for Color {
    async fn deserialize<T>(data: &mut T) -> io::Result<Self>
    where
        Self: Sized,
        T: AsyncReadExt + Unpin + Send,
    {
        Ok(match data.read_u8().await? {
            0 => Self::Reset,
            1 => Self::Black,
            2 => Self::DarkGrey,
            3 => Self::Red,
            4 => Self::DarkRed,
            5 => Self::Green,
            6 => Self::DarkGreen,
            7 => Self::Yellow,
            8 => Self::DarkYellow,
            9 => Self::Blue,
            10 => Self::DarkBlue,
            11 => Self::Magenta,
            12 => Self::DarkMagenta,
            13 => Self::Cyan,
            14 => Self::DarkCyan,
            15 => Self::White,
            16 => Self::Grey,
            17 => {
                let r = data.read_u8().await?;
                let g = data.read_u8().await?;
                let b = data.read_u8().await?;
                Self::Rgb { r, g, b }
            }
            18 => Self::AnsiValue(data.read_u8().await?),
            _ => unreachable!(),
        })
    }
}

impl<T> Serialize for Vec<T>
where
    T: Serialize,
{
    fn serialize(&self) -> Vec<u8> {
        let mut ret = Vec::new();
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
        println!("{size}");
        let mut ret = Self::with_capacity(size);
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
    fn serialize(&self) -> Vec<u8> {
        let mut ret = Vec::new();
        ret.extend((self.len() as u64).to_be_bytes());
        for elem in self {
            ret.extend(elem.serialize());
        }
        ret
    }
}

impl Serialize for &str {
    fn serialize(&self) -> Vec<u8> {
        let mut ret = Vec::new();
        ret.extend((self.len() as u64).to_be_bytes());
        ret.extend(self.as_bytes());
        ret
    }
}

impl Serialize for String {
    fn serialize(&self) -> Vec<u8> {
        self.as_str().serialize()
    }
}

impl Deserialize for String {
    async fn deserialize<T>(data: &mut T) -> io::Result<Self>
    where
        Self: Sized,
        T: AsyncReadExt + Unpin + Send,
    {
        let len = data.read_u64().await? as usize;
        let mut buf = vec![0; len];
        data.read_exact(&mut buf).await?;
        Ok(String::from_utf8(buf).expect("Invalid utf was sent"))
    }
}
