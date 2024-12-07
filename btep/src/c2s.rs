//! mdoule for client updates sendt to the server

use std::{collections::VecDeque, io};

use tokio::io::AsyncReadExt;
use utils::other::CursorPos;

use crate::{Deserialize, Serialize};

/// S2C or Server to Client
/// Encodes information that originates from the client and sendt to the server
#[derive(Clone, Copy, Debug)]
pub enum C2S {
    /// The client wrote a character
    Char(char),
    /// The client pressed backspace
    Backspace,
    /// The client pressed enter
    Enter,
    /// The client pressed entered insert mode at a position
    // TODO: this should use the `EnterInsert` instead which should be more immune to server-client
    // desync
    EnterInsert(CursorPos),
    /// Force a save to happen
    Save,
}

#[derive(Clone, Copy, Debug)]
/// A representation of entering insert mode which shuold be more accurate than just sending the
/// clients cursors position
pub struct EnterInsert {
    /// The id of the buffer that was split
    pub id: usize,
    /// The offset in that buffer
    /// Option because appending a character has special behaviour
    pub offset: Option<usize>,
}

impl Serialize for EnterInsert {
    fn serialize(&self) -> VecDeque<u8> {
        todo!()
    }
}

impl Deserialize for EnterInsert {
    async fn deserialize<T>(_data: &mut T) -> io::Result<Self>
    where
        T: AsyncReadExt,
        Self: Sized,
    {
        todo!()
    }
}

impl Serialize for C2S {
    fn serialize(&self) -> VecDeque<u8> {
        match self {
            Self::Char(c) => std::iter::once(1).chain(c.serialize()).collect(),
            Self::EnterInsert(a) => std::iter::once(2).chain(a.serialize()).collect(),
            Self::Enter => [10].into(),
            Self::Backspace => [8].into(),
            Self::Save => [3].into(),
        }
    }
}

impl Deserialize for C2S {
    async fn deserialize<T>(data: &mut T) -> io::Result<Self>
    where
        T: AsyncReadExt + Unpin + Send,
        Self: Sized,
    {
        Ok(match data.read_u8().await? {
            1 => Self::Char(
                char::from_u32(data.read_u32().await?).expect("An invalid char was supplied"),
            ),
            2 => Self::EnterInsert(CursorPos::deserialize(data).await?),
            3 => Self::Save,
            8 => Self::Backspace,
            10 => Self::Enter,
            x => unreachable!("{x}"),
        })
    }
}
