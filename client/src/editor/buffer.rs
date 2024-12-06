use std::{
    cmp,
    io::{Read, Write},
};

use btep::{c2s::C2S, s2c::S2C};
use text::Text;
use tungstenite::WebSocket;
use utils::other::CursorPos;

#[derive(Debug)]
/// The main state for the entire editor. The entireity of the
/// view presented to the user can be rebuild from this
pub struct Buffer<T> {
    /// The rope stores the entire file being edited.
    pub text: Text,
    /// Our own id within the Text
    pub(super) id: usize,
    /// stores where the cursor is located
    pub(crate) cursorpos: CursorPos,
    /// stores the amount of lines that have been scrolled down
    pub(crate) line_offset: usize,
    pub(crate) socket: Option<WebSocket<T>>,
}

impl<T> Buffer<T> {
    /// Creates a new appstate
    #[must_use]
    pub fn new(mut text: Text, socket: Option<WebSocket<T>>) -> Self {
        let id = text.add_client();
        Self {
            text,
            id,
            cursorpos: CursorPos::default(),
            line_offset: 0,
            socket,
        }
    }

    /// Returns an immutable reference to the internal
    /// cursors position
    #[must_use]
    pub const fn cursor(&self) -> &CursorPos {
        &self.cursorpos
    }

    /// save the current buffer
    pub(super) fn save(&mut self)
    where
        T: Read + Write,
    {
        if let Some(ref mut socket) = self.socket {
            socket.write(C2S::Save.into()).unwrap();
            socket.flush().unwrap();
        }
    }

    /// Fetches the network for any updates and updates the internal buffer accordingly
    /// # Return value
    /// returns true if the screen should be redrawn
    /// # Panics
    /// the message received wasn't formatted properly
    pub fn update(&mut self) -> bool
    where
        T: Read + Write,
    {
        let Some(ref mut socket) = self.socket else {
            return false;
        };
        if let Ok(msg) = socket.read() {
            match S2C::<Text>::from_message(msg).unwrap() {
                S2C::Full(_) => unreachable!("A full buffer shouldn't be sent"),
                S2C::Update((client_id, action)) => {
                    let client = self.text.client(client_id);
                    match action {
                        C2S::Char(c) => {
                            client.push_char(c);
                            if client.data.as_ref().unwrap().pos.row == self.cursorpos.row
                                && client.data.as_ref().unwrap().pos.col < self.cursorpos.col
                            {
                                self.cursorpos.col += 1;
                            }
                        }
                        C2S::Backspace => {
                            if let Some(del_char) = client.backspace() {
                                if client.data.as_ref().unwrap().pos.row == self.cursorpos.row
                                    && client.data.as_ref().unwrap().pos.col < self.cursorpos.col
                                {
                                    self.cursorpos.col -= 1;
                                }
                                if del_char == '\n'
                                    && client.data.as_ref().unwrap().pos.row < self.cursorpos.row
                                {
                                    self.cursorpos.row -= 1;
                                }
                            }
                        }
                        C2S::Enter => {
                            client.push_char('\n');
                            match client
                                .data
                                .as_ref()
                                .unwrap()
                                .pos
                                .row
                                .cmp(&self.cursorpos.row)
                            {
                                cmp::Ordering::Less => {
                                    self.cursorpos.row += 1;
                                }
                                cmp::Ordering::Equal => {
                                    if client.data.as_ref().unwrap().pos.col < self.cursorpos.col {
                                        self.cursorpos.row += 1;
                                        self.cursorpos.col = 0;
                                    }
                                }
                                cmp::Ordering::Greater => (),
                            }
                        }
                        C2S::EnterInsert(pos) => drop(client.enter_insert(pos)),
                        C2S::Save => unreachable!(),
                    };
                    true
                }
                S2C::NewClient => {
                    self.text.add_client();
                    false
                }
            }
        } else {
            false
        }
    }

    pub fn recalculate_cursor(&mut self, (_cols, rows): (u16, u16)) {
        if self.line_offset > self.cursorpos.row {
            self.line_offset = self.cursorpos.row;
        } else if self.line_offset + usize::from(rows) <= self.cursorpos.row {
            self.line_offset = self.cursorpos.row - usize::from(rows) + 1;
        }
    }
}
