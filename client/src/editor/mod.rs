//! This is the main module for handling editor stuff.
//! This includes handling keypressess and adding these
//! to the queue for sending to the server, but *not*
//! actually sending them


use buffer::Buffer;
use text::Text;
use tungstenite::WebSocket;
mod buffer;
mod draw;

#[derive(Default)]
pub struct Client<T> {
    buffers: Vec<Buffer<T>>,
    current_buffer: usize,
}

impl<T> Client<T> {
    pub fn new() -> Self {
        let buf = Buffer::<T>::new(Text::new(), None);
        Self {
            buffers: Vec::from([buf]),
            current_buffer: 0,
        }
    }

    pub fn new_with_buffer(text: Text, socket: Option<WebSocket<T>>) -> Self {
        let buf = Buffer::<T>::new(text, socket);
        Self {
            buffers: Vec::from([buf]),
            current_buffer: 0,
        }
    }

    pub fn curr(&mut self) -> &mut Buffer<T> {
        &mut self.buffers[self.current_buffer]
    }
}

/// Stores the current mode of the editor.
/// These work in the same way as vims modes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command(String),
}
