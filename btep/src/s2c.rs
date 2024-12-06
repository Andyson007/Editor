//! Communication from the server to the client
use std::{collections::VecDeque, mem};
use tungstenite::Message;
use utils::iters::IteratorExt;
use {crate::c2s::C2S, crate::Deserialize, crate::Serialize};

/// S2C or Server to Client
/// Encodes information that originates from the server and sendt to the client
pub enum S2C<T> {
    /// The initial message over the websocket.
    /// This should usually be the entireity of the file
    Full(T),
    /// A client has made an update to their buffer
    Update((usize, C2S)),
    /// A client has connected
    NewClient,
}

impl<T> S2C<T> {
    /// Converts a bytestream into a message
    #[must_use]
    pub fn into_message(self) -> Message
    where
        T: Serialize,
    {
        let mut serialized = self.serialize();
        match self {
            Self::Full(_) => {
                serialized.push_front(0);
            }
            Self::Update(_) => {
                serialized.push_front(1);
            }
            Self::NewClient => serialized.push_front(2),
        }
        Message::Binary(serialized.into())
    }
}

impl<T> Serialize for S2C<T>
where
    T: Serialize,
{
    fn serialize(&self) -> VecDeque<u8> {
        let mut ret = VecDeque::new();
        match self {
            Self::Full(x) => {
                ret.extend(x.serialize());
            }
            Self::Update((id, action)) => {
                ret.extend((*id as u64).to_be_bytes());
                ret.extend(action.serialize());
            }
            Self::NewClient => (),
        };
        ret
    }
}

impl<T> Deserialize for S2C<T>
where
    T: Deserialize,
{
    fn deserialize(data: &[u8]) -> Self {
        let mut iter = data.iter();
        match iter.next().unwrap() {
            0 => Self::Full(T::deserialize(&data[1..])),
            1 => {
                let id = u64::from_be_bytes(
                    iter.by_ref()
                        .copied()
                        .chunks::<{ mem::size_of::<u64>() }>()
                        .next()
                        .unwrap(),
                ) as usize;
                let action = C2S::deserialize(&iter.copied().collect::<Vec<_>>());
                Self::Update((id, action))
            }
            2 => Self::NewClient,
            _ => panic!("An invalid specifier was found"),
        }
    }
}
