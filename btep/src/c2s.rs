//! mdoule for client updates sendt to the server

use std::{collections::VecDeque, mem};

use tungstenite::Message;
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
    fn deserialize(_data: &[u8]) -> Self {
        todo!()
    }
}

impl Serialize for C2S {
    fn serialize(&self) -> VecDeque<u8> {
        match self {
            Self::Char(c) => std::iter::once(1)
                .chain(c.serialize())
                .collect(),
            Self::EnterInsert(a) => std::iter::once(2)
                .chain(a.serialize())
                .collect(),
            Self::Enter => [10].into(),
            Self::Backspace => [8].into(),
            Self::Save => [3].into(),
        }
    }
}

impl Deserialize for C2S {
    fn deserialize(data: &[u8]) -> Self {
        let mut iter = data.iter();
        match iter.next().unwrap() {
            1 => Self::Char(
                char::from_u32(u32::from_be_bytes(
                    iter.copied()
                        .next_chunk::<{ mem::size_of::<u32>() }>()
                        .unwrap(),
                ))
                .expect("An invalid char was supplied"),
            ),
            2 => Self::EnterInsert(CursorPos::deserialize(&data[1..])),
            3 => Self::Save,
            8 => Self::Backspace,
            10 => Self::Enter,
            _ => unreachable!(),
        }
    }
}

impl From<C2S> for Message {
    fn from(value: C2S) -> Self {
        Self::Binary(value.serialize().into())
    }
}
