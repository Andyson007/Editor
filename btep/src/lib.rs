//! This crate implements a custom binary
//! text transfer protocol.

use core::str;
use std::{io::Read, iter};

use tungstenite::Message;
/// Btep or the Binary Text Editor Protocol
pub enum Btep<T> {
    Initial(T),
}

impl Btep<Box<[u8]>> {
    pub fn as_utf8(&self) -> &str {
        match self {
            Btep::Initial(x) => str::from_utf8(x).unwrap(),
        }
    }
}

impl<T> IntoMessage for Btep<T>
where
    T: IntoIterator<Item = u8>,
{
    fn into_message(self) -> Message {
        match self {
            Btep::Initial(x) => Message::Binary(iter::once(0).chain(x).collect::<Vec<_>>()),
        }
    }
}

impl FromMessage for Btep<Box<[u8]>> {
    fn from_message(msg: Message) -> Self {
        let Message::Binary(data) = msg else {
            panic!("wrong message type")
        };
        match data[0] {
            0 => Btep::Initial(data[1..].into()),
            _ => panic!("An invalid specifier was found"),
        }
    }
}

pub trait IntoMessage {
    fn into_message(self) -> Message;
}

pub trait FromMessage {
    fn from_message(msg: Message) -> Self;
}
