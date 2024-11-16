//! This crate implements a custom binary
//! text transfer protocol.

use std::iter;

use tungstenite::Message;
/// Btep or the Binary Text Editor Protocol
pub enum Btep<T> {
    /// The initial message over the websocket.
    /// This should usually be the entireity of the file
    Full(T),
}

impl<T> Btep<T>
where
    T: Serialize,
{
    /// Converts a bytestream into a message
    #[must_use]
    pub fn into_message(self) -> Message {
        match self {
            Self::Full(x) => {
                Message::Binary(iter::once(0).chain(x.serialize()).collect::<Vec<_>>())
            }
        }
    }
}

impl<T> Btep<T> {
    /// Converts a message into a byte array
    ///
    /// # Panics
    /// This method panics when the message is empty,
    /// and when it has an invalid format identifier
    #[must_use]
    pub fn from_message(msg: Message) -> Self
    where
        T: Deserialize,
    {
        let Message::Binary(data) = msg else {
            panic!("wrong message type")
        };
        match data.first().unwrap() {
            0 => Self::Full(Deserialize::deserialize(data.into_iter().skip(1))),
            _ => panic!("An invalid specifier was found"),
        }
    }
}

pub trait Serialize {
    fn serialize(&self) -> impl IntoIterator<Item = u8>;
}

pub trait Deserialize {
    fn deserialize(data: impl IntoIterator<Item = u8>) -> Self;
}

// impl<'a, T: Serialize + 'a, U: Deref<Target = T>> Serialize for U {
//     fn serialize(&'a self) -> impl IntoIterator<Item = u8> {
//         self.deref().serialize()
//     }
// }
