//! mdoule for client updates sendt to the server

use std::collections::VecDeque;

use tungstenite::Message;

use crate::{Deserialize, Serialize};

/// S2C or Server to Client
/// Encodes information that originates from the client and sendt to the server
pub enum C2S {
    Char(char),
    Backspace,
    Enter,
    Infallible(!),
}

impl Serialize for C2S {
    fn serialize(&self) -> VecDeque<u8> {
        match self {
            C2S::Char(c) => std::iter::once(1)
                .chain((*c as u32).to_be_bytes())
                .collect(),
            C2S::Enter => [10].into(),
            C2S::Backspace => [8].into(),
            C2S::Infallible(_) => todo!(),
        }
    }
}

impl Deserialize for C2S {
    fn deserialize(data: &[u8]) -> Self {
        let mut iter = data.iter();
        match iter.next().unwrap() {
            1 => Self::Char(
                char::from_u32(u32::from_be_bytes(iter.copied().next_chunk::<4>().unwrap()))
                    .expect("An invalid char was supplied"),
            ),
            8 => Self::Backspace,
            _ => unreachable!(),
        }
    }
}

impl From<C2S> for Message {
    fn from(value: C2S) -> Self {
        Message::Binary(value.serialize().into())
    }
}
