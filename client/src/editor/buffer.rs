use std::{
    cmp,
    io::{Read, Write},
};

use btep::{c2s::C2S, s2c::S2C, Deserialize, Serialize};
use text::Text;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use utils::other::CursorPos;

#[derive(Debug)]
/// The main state for the entire editor. The entireity of the
/// view presented to the user can be rebuild from this
pub struct Buffer {
    /// The rope stores the entire file being edited.
    pub text: Text,
    /// Our own id within the Text
    pub(super) id: usize,
    /// stores where the cursor is located
    pub(crate) cursorpos: CursorPos,
    /// stores the amount of lines that have been scrolled down
    pub(crate) line_offset: usize,
    pub(crate) socket: Option<TcpStream>,
}

impl Buffer {
    /// Creates a new appstate
    #[must_use]
    pub fn new(mut text: Text, socket: Option<TcpStream>) -> Self {
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
    pub(super) async fn save(&mut self) -> tokio::io::Result<()> {
        if let Some(ref mut socket) = self.socket {
            socket
                .write_all(C2S::Save.serialize().make_contiguous())
                .await?;

            socket.flush().await?;
        }
        Ok(())
    }

    /// Fetches the network for any updates and updates the internal buffer accordingly
    /// # Return value
    /// returns true if the screen should be redrawn
    /// # Panics
    /// the message received wasn't formatted properly
    pub async fn update(&mut self) -> bool {
        let Some(ref mut socket) = self.socket else {
            return false;
        };
        let mut msg = Vec::new();
        socket.read_buf(&mut msg).await.unwrap();

        match S2C::<Text>::deserialize(&msg) {
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
    }

    pub fn recalculate_cursor(&mut self, (_cols, rows): (u16, u16)) {
        if self.line_offset > self.cursorpos.row {
            self.line_offset = self.cursorpos.row;
        } else if self.line_offset + usize::from(rows) <= self.cursorpos.row {
            self.line_offset = self.cursorpos.row - usize::from(rows) + 1;
        }
    }
}
