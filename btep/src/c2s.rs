//! mdoule for client updates sendt to the server

use std::{collections::VecDeque, mem};

use tungstenite::Message;

use crate::{Deserialize, Serialize};

/// S2C or Server to Client
/// Encodes information that originates from the client and sendt to the server
#[derive(Clone, Copy, Debug)]
pub enum C2S {
    Char(char),
    Backspace,
    Enter,
    EnterInsert(usize),
}

#[derive(Clone, Copy, Debug)]
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
    fn deserialize(data: &[u8]) -> Self {
        todo!()
    }
}

impl Serialize for C2S {
    fn serialize(&self) -> VecDeque<u8> {
        match self {
            C2S::Char(c) => std::iter::once(1)
                .chain((*c as u32).to_be_bytes())
                .collect(),
            C2S::EnterInsert(a) => std::iter::once(2)
                .chain((*a as u64).to_be_bytes())
                .collect(),
            C2S::Enter => [10].into(),
            C2S::Backspace => [8].into(),
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
            2 => Self::EnterInsert(u64::from_be_bytes(
                iter.copied()
                    .next_chunk::<{ mem::size_of::<u64>() }>()
                    .unwrap(),
            ) as usize),
            8 => Self::Backspace,
            10 => Self::Enter,
            _ => unreachable!(),
        }
    }
}

impl From<C2S> for Message {
    fn from(value: C2S) -> Self {
        Message::Binary(value.serialize().into())
    }
}
