use std::{io, net::SocketAddrV4, path::PathBuf};

use btep::{
    c2s::C2S,
    s2c::{Inhabitant, S2C},
    Deserialize, Serialize,
};

use crossterm::{style::Color, terminal};
use text::Text;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
};
use utils::other::CursorPos;

use super::BUFFER_SIZE;

/// The main state for the entire editor. The entireity of the
/// view presented to the user can be rebuild from this
#[derive(Debug)]
pub struct Buffer {
    pub data: BufferData,
    /// stores where the cursor is located
    pub(crate) cursorpos: CursorPos,
    /// stores the amount of lines that have been scrolled down
    pub(crate) line_offset: usize,
    pub(crate) socket: Option<Socket>,
    pub path: Option<PathBuf>,
}

#[derive(Debug)]
pub struct BufferData {
    pub buffer_type: BufferTypeData,
    pub modifiable: bool,
}

#[derive(Debug)]
pub enum BufferTypeData {
    Regular {
        /// Our own id within the Text
        id: usize,
        /// The rope stores the entire file being edited.
        text: Text,
        /// An map from id to their color in the buffer
        colors: Vec<Color>,
    },
    Folder {
        inhabitants: Vec<Inhabitant>,
    },
}

#[derive(Debug)]
pub struct Socket {
    pub reader: OwnedReadHalf,
    pub writer: OwnedWriteHalf,
}

impl Buffer {
    pub async fn connect<S: AsRef<str>, P: Into<PathBuf>>(
        address: SocketAddrV4,
        username: &str,
        password: Option<S>,
        path: P,
    ) -> io::Result<Self> {
        let Ok(mut socket) = connect_with_auth(address, username, password).await else {
            panic!("Failed to connect to the server. Maybe the server is not running?")
        };
        let path_buf = path.into();
        socket
            .write_all(&C2S::Path(path_buf.clone()).serialize())
            .await?;
        match S2C::<Text>::deserialize(&mut BufReader::with_capacity(BUFFER_SIZE, &mut socket))
            .await?
        {
            S2C::Full(initial_text) => {
                let colors = Vec::<Color>::deserialize(&mut socket).await?;
                let buf = Buffer::new(username, initial_text, colors, Some(socket), Some(path_buf));
                Ok(buf)
            }
            S2C::Folder(inhabitants) => Ok(Buffer::new_folder(inhabitants, path_buf)),
            _ => panic!("Initial message in wrong protocol"),
        }
    }
    /// Creates a new appstate
    #[must_use]
    pub fn new<P: Into<PathBuf>>(
        username: &str,
        mut text: Text,
        colors: Vec<Color>,
        socket: Option<TcpStream>,
        path: Option<P>,
    ) -> Self {
        let id = text.add_client(username);
        Self {
            path: path.map(|x| x.into()),
            data: BufferData {
                buffer_type: BufferTypeData::Regular { text, colors, id },
                modifiable: true,
            },
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
    pub fn new_folder<P: Into<PathBuf>>(inhabitants: Vec<Inhabitant>, path: P) -> Self {
        Self {
            data: BufferData {
                buffer_type: BufferTypeData::Folder { inhabitants },
                modifiable: false,
            },
            cursorpos: CursorPos::default(),
            line_offset: 0,
            socket: None,
            path: Some(path.into()),
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
                let BufferTypeData::Regular { text, .. } = &mut self.data.buffer_type else {
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
                let BufferTypeData::Regular { text, colors, .. } = &mut self.data.buffer_type
                else {
                    panic!("New clients cannot join non-regular files");
                };
                text.add_client(&username);
                colors.push(color);
                Ok(false)
            }
        }
    }

    /// Recalculates the cursor position using the size of the terminal
    pub fn recalculate_cursor(&mut self, (_cols, rows): (u16, u16)) -> io::Result<()> {
        let size = terminal::size()?;
        match &mut self.data.buffer_type {
            BufferTypeData::Regular { text, .. } => {
                if self.line_offset > self.cursorpos.row {
                    self.line_offset = self.cursorpos.row;
                } else {
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
                    if self.line_offset + usize::from(rows) <= self.cursorpos.row + cursor_offset {
                        self.line_offset += self.cursorpos.row + cursor_offset
                            - self.line_offset
                            - usize::from(rows)
                            + 1;
                    }
                }
                Ok(())
            }
            BufferTypeData::Folder { inhabitants } => {
                if self.line_offset > self.cursorpos.row {
                    self.line_offset = self.cursorpos.row;
                } else {
                    let cursor_offset = || todo!();
                    if self.line_offset + usize::from(rows) <= self.cursorpos.row {
                        self.line_offset +=
                            self.cursorpos.row - self.line_offset - usize::from(rows) + 1;
                    }
                }
                Ok(())
            }
        }
    }
}

async fn connect_with_auth<S: AsRef<str>>(
    address: SocketAddrV4,
    username: &str,
    password: Option<S>,
) -> io::Result<TcpStream> {
    let mut stream = TcpStream::connect(address).await?;
    stream.write_all(username.as_bytes()).await?;
    if let Some(password) = password {
        stream.write_u8(254).await?;
        stream.write_all(password.as_ref().as_bytes()).await?;
    }
    stream.write_all(&[255]).await?;
    stream.flush().await?;
    let ret = stream.read_u8().await?;
    match ret {
        0 => (),
        1 => panic!("You forgot to include a password"),
        2 => panic!("The username, password combination you supplied isn't authorized"),
        3 => {
            panic!("This shouldn't be reachable, but it means that you forgot to supply a password")
        }
        _ => unreachable!(),
    }
    Ok(stream)
}
