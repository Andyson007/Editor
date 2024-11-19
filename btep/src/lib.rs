//! This crate implements a custom binary
//! text transfer protocol.

use std::collections::VecDeque;

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
        let mut serialized = self.serialize();
        match self {
            Self::Full(_) => {
                serialized.push_front(0);
                Message::Binary(serialized.into())
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
            0 => Self::Full(Deserialize::deserialize(&data[1..])),
            _ => panic!("An invalid specifier was found"),
        }
    }
}

impl<T> Serialize for Btep<T>
where
    T: Serialize,
{
    fn serialize(&self) -> VecDeque<u8> {
        match self {
            Self::Full(x) => x.serialize(),
        }
    }
}

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

// impl<'a, T: Serialize + 'a, U: Deref<Target = T>> Serialize for U {
//     fn serialize(&'a self) -> impl IntoIterator<Item = u8> {
//         self.deref().serialize()
//     }
// }
