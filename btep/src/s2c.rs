use crate::c2s::C2S;
use crate::Deserialize;
use crate::Serialize;

use std::collections::VecDeque;
use tungstenite::Message;

/// S2C or Server to Client
/// Encodes information that originates from the server and sendt to the client
pub enum S2C<T> {
    /// The initial message over the websocket.
    /// This should usually be the entireity of the file
    Full(T),
    Update((usize, C2S)),
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
        }
        Message::Binary(serialized.into())
    }
}

impl<T> S2C<T> {
    /// Converts a message into a byte array
    ///
    /// # Panics
    /// The first byte didn't specify a valid format
    #[must_use]
    pub fn from_message(msg: Message) -> Option<Self>
    where
        T: Deserialize,
    {
        let Message::Binary(data) = msg else {
            panic!("wrong message type")
        };
        match data.first()? {
            0 => Some(Self::Full(T::deserialize(&data[1..]))),
            _ => panic!("An invalid specifier was found"),
        }
    }
}

impl<T> Serialize for S2C<T>
where
    T: Serialize,
{
    fn serialize(&self) -> VecDeque<u8> {
        match self {
            Self::Full(x) => x.serialize(),
            Self::Update((id, action)) => {
                let mut ret: VecDeque<u8> = (*id as u64).to_be_bytes().into();
                ret.extend(action.serialize());
                ret
            }
        }
    }
}

impl<T> From<S2C<T>> for Message
where
    T: Serialize,
{
    fn from(value: S2C<T>) -> Self {
        Message::Binary(value.serialize().into())
    }
}
