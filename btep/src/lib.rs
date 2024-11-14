//! This crate implements a custom binary
//! text transfer protocol.

use std::iter;

use tungstenite::Message;
/// Btep or the Binary Text Editor Protocol
pub enum Btep<T> {
    /// The initial message over the websocket.
    /// This should usually be the entireity of the file
    Initial(T),
}

impl<T> Btep<T>
where
    T: IntoIterator<Item = u8>,
{
    /// Converts a bytestream into a message
    #[must_use]
    pub fn into_message(self) -> Message {
        match self {
            Self::Initial(x) => Message::Binary(iter::once(0).chain(x).collect::<Vec<_>>()),
        }
    }
}

impl Btep<Box<[u8]>> {
    /// Converts a message into a byte array
    ///
    /// # Panics
    /// This method panics when the message is empty,
    /// and when it has an invalid format identifier
    #[must_use]
    pub fn from_message(msg: Message) -> Self {
        let Message::Binary(data) = msg else {
            panic!("wrong message type")
        };
        match data.first().unwrap() {
            0 => Self::Initial(data[1..].into()),
            _ => panic!("An invalid specifier was found"),
        }
    }
}
