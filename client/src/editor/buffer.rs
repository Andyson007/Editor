use std::{ffi::OsString, io};

use btep::{
    c2s::C2S,
    s2c::{Inhabitant, S2C},
    Deserialize, Serialize,
};
use crossterm::{style::Color, terminal};
use text::Text;
use tokio::{
    io::AsyncWriteExt,
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};
use utils::other::CursorPos;

/// The main state for the entire editor. The entireity of the
/// view presented to the user can be rebuild from this
#[derive(Debug)]
pub struct Buffer {
    pub data: BufferData,
    /// Our own id within the Text
    pub(super) id: usize,
    /// stores where the cursor is located
    pub(crate) cursorpos: CursorPos,
    /// stores the amount of lines that have been scrolled down
    pub(crate) line_offset: usize,
    pub(crate) socket: Option<Socket>,
}

#[derive(Debug)]
pub enum BufferData {
    Regular {
        /// The rope stores the entire file being edited.
        text: Text,
        /// An map from id to their color in the buffer
        colors: Vec<Color>,
    },
    Folder {
        path: OsString,
    },
}

#[derive(Debug)]
pub struct Socket {
    pub reader: OwnedReadHalf,
    pub writer: OwnedWriteHalf,
}

impl Buffer {
    /// Creates a new appstate
    #[must_use]
    pub fn new(
        username: String,
        mut text: Text,
        colors: Vec<Color>,
        socket: Option<TcpStream>,
    ) -> Self {
        let id = text.add_client(&username);
        Self {
            data: BufferData::Regular { text, colors },
            id,
            cursorpos: CursorPos::default(),
            line_offset: 0,
            socket: socket.map(|x| {
                let (read, writer) = x.into_split();
                Socket {
                    reader: read,
                    writer,
                }
            }),
        }
    }

    #[must_use]
    pub fn new_folder(username: String, inhabitants: Vec<Inhabitant>) -> Self {
        todo!()
        // Self {
        //     text: todo!(),
        //     colors: todo!(),
        //     id: 0, // Doesn't matter
        //     cursorpos: CursorPos::default(),
        //     line_offset: 0,
        //     socket: None,
        // }
    }

    /// Returns an immutable reference to the internal
    /// cursors position
    #[must_use]
    pub const fn cursor(&self) -> &CursorPos {
        &self.cursorpos
    }

    /// save the current buffer
    pub(super) async fn save(&mut self) -> tokio::io::Result<()> {
        if let Some(Socket { ref mut writer, .. }) = self.socket {
            writer.write_all(&C2S::Save.serialize()).await?;

            writer.flush().await?;
        }
        Ok(())
    }

    /// Fetches the network for any updates and updates the internal buffer accordingly
    /// # Return value
    /// returns true if the screen should be redrawn
    /// # Panics
    /// the message received wasn't formatted properly
    pub async fn update(&mut self) -> io::Result<bool> {
        let Some(Socket { ref mut reader, .. }) = self.socket else {
            return Ok(false);
        };

        match S2C::<Text>::deserialize(reader).await? {
            S2C::Full(_) => unreachable!("A full buffer shouldn't be sent"),
            S2C::Folder(_) => unreachable!("A folder shouldn't be sent"),
            S2C::Update((client_id, action)) => {
                let BufferData::Regular { text, .. } = &mut self.data else {
                    panic!("Only updates in Regul mode are supported (at the moment)");
                };
                let client = text.client_mut(client_id);
                match action {
                    C2S::Char(c) => {
                        client.push_char(c);
                    }
                    C2S::Backspace(swaps) => {
                        client.backspace_with_swaps(swaps);
                    }
                    C2S::Enter => {
                        client.push_char('\n');
                    }
                    C2S::EnterInsert(pos) => drop(client.enter_insert(pos)),
                    C2S::ExitInsert => client.exit_insert(),
                    C2S::Save | C2S::Path(_) => unreachable!(),
                };
                Ok(true)
            }
            S2C::NewClient((username, color)) => {
                let BufferData::Regular { text, colors } = &mut self.data else {
                    panic!("New clients cannot join non-regular files");
                };
                text.add_client(&username);
                colors.push(color);
                Ok(false)
            }
        }
    }

    pub fn recalculate_cursor(&mut self, (_cols, rows): (u16, u16)) -> io::Result<()> {
        let size = terminal::size()?;
        let BufferData::Regular { text, .. } = &mut self.data else {
            todo!("Cursor locations exist in folders too")
        };

        let mut current_line = 0;
        let mut relative_col = 0;
        let mut cursor_offset = 0;
        'outer: for buf in text.bufs() {
            let read_lock = buf.read();
            for c in read_lock.text.chars() {
                if c == '\n' {
                    relative_col = 0;
                    if current_line >= size.1 as usize + self.line_offset {
                        break 'outer;
                    };
                    current_line += 1;
                } else if current_line >= self.line_offset {
                    if relative_col >= size.0 as usize - 3 {
                        relative_col = 0;
                        current_line += 1;
                        if self.cursor().row - self.line_offset >= current_line {
                            cursor_offset += 1;
                        }
                    } else {
                        relative_col += 1;
                    }
                }
            }
        }
        if self.line_offset > self.cursorpos.row {
            self.line_offset = self.cursorpos.row;
        } else if self.line_offset + usize::from(rows) <= self.cursorpos.row + cursor_offset {
            self.line_offset +=
                self.cursorpos.row + cursor_offset - self.line_offset - usize::from(rows) + 1;
        }
        Ok(())
    }
}
