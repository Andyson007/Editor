use crate::editor::buffer;
use std::fmt::Debug;
use std::net::SocketAddrV4;
use std::{cmp, io, path::Path};
use tokio::{io::AsyncWriteExt, net::TcpStream};

use btep::{c2s::C2S, Serialize};
use crossterm::{event::KeyEvent, style::Color};
use text::Text;
use utils::other::CursorPos;

use crate::editor::buffer::Buffer;

use super::buffer::BufferTypeData;
/// Represents a single client.
pub struct Client {
    #[cfg(feature = "security")]
    pub(crate) password: String,
    pub(crate) username: String,
    pub(crate) color: Color,
    pub(crate) server_addr: SocketAddrV4,
    /// All the buffers the client is connected to
    pub buffers: Vec<Buffer>,
    /// The buffer that the client should currently be showing
    pub current_buffer: usize,
    /// Stores the current editing mode. This is
    /// effectively the same as Vims insert/Normal mode
    pub(crate) modeinfo: ModeInfo,
    /// Stores a message that should be rendered to the user
    pub(crate) info: Option<String>,
}

impl Client {
    /// Cretaes a new client an empty original buffer
    pub async fn from_path(
        username: String,
        #[cfg(feature = "security")] password: String,
        address: SocketAddrV4,
        color: &Color,
        path: &Path,
    ) -> io::Result<Self> {
        Ok(Self {
            server_addr: address,
            username: username.clone(),
            #[cfg(feature = "security")]
            password: password.clone(),
            buffers: vec![
                Buffer::connect(
                    address,
                    &username,
                    #[cfg(feature = "security")]
                    password,
                    color,
                    path,
                )
                .await?,
            ],
            current_buffer: 0,
            modeinfo: ModeInfo::default(),
            color: color.to_owned(),
            info: Some("Press Escape then :help to view help".to_string()),
        })
    }

    /// returns the current buffer that should be visible
    #[must_use]
    pub fn curr(&self) -> &Buffer {
        &self.buffers[self.current_buffer]
    }

    /// returns the current buffer that should be visible
    pub fn curr_mut(&mut self) -> &mut Buffer {
        &mut self.buffers[self.current_buffer]
    }

    /// Executed a command written in command mode
    pub(crate) async fn execute_command(&mut self, cmd: &str) -> io::Result<bool> {
        match cmd {
            "q" => return Ok(self.close_current_buffer()),
            "w" => self.curr_mut().save().await?,
            "help" => self.add_buffer(
                "Doesn't matter".to_string(),
                Text::original_from_str(include_str!("../../../help")),
                Vec::new(),
                None,
                None,
            ),
            "bn" | "bufnext" => {
                self.current_buffer = (self.current_buffer + 1) % self.buffers.len()
            }
            "bp" | "bufprev" | "bufprevious" => {
                self.current_buffer =
                    (self.current_buffer + self.buffers.len() + 1) % self.buffers.len()
            }
            _ => (),
        }
        Ok(false)
    }

    /// adds a buffer and switches to it
    fn add_buffer(
        &mut self,
        username: String,
        text: Text,
        colors: Vec<Color>,
        socket: Option<TcpStream>,
        path: Option<&Path>,
    ) {
        self.buffers
            .push(Buffer::new(&username, text, colors, socket, path));
        self.current_buffer = self.buffers.len() - 1;
    }

    fn close_current_buffer(&mut self) -> bool {
        self.buffers.remove(self.current_buffer);
        if self.current_buffer == self.buffers.len() {
            if self.buffers.is_empty() {
                return true;
            }
            self.current_buffer -= 1;
        }
        false
    }

    /// types a char in insert mode
    /// This function handles sending the request *without* flushing the stream.
    /// Cursor movement is also handled
    pub(crate) async fn type_char(&mut self, c: char) -> io::Result<()> {
        let BufferTypeData::Regular {
            ref mut text,
            id: curr_id,
            ..
        } = self.curr_mut().data.buffer_type
        else {
            todo!("You can only type in regular buffers")
        };
        text.client_mut(curr_id).push_char(c);
        match c {
            '\n' => {
                self.curr_mut().cursorpos.col = 0;
                self.curr_mut().cursorpos.row += 1;
            }
            _ => self.curr_mut().cursorpos.col += 1,
        }
        if let Some(buffer::Socket { ref mut writer, .. }) = self.curr_mut().socket {
            writer.write_all(&C2S::Char(c).serialize()).await?
        }
        Ok(())
    }

    pub(crate) async fn exit_insert(&mut self) -> io::Result<()> {
        let BufferTypeData::Regular {
            ref mut text,
            id: curr_id,
            ..
        } = self.curr_mut().data.buffer_type
        else {
            unreachable!("You can only be in insert mode in regular buffers");
        };
        text.client_mut(curr_id).exit_insert();
        self.modeinfo.set_mode(Mode::Normal);

        if let Some(buffer::Socket { ref mut writer, .. }) = self.curr_mut().socket {
            writer.write_all(&C2S::ExitInsert.serialize()).await?
        }
        let BufferTypeData::Regular { text, .. } = &self.curr().data.buffer_type else {
            todo!()
        };
        let curr_line_len = text.lines().nth(self.curr().cursorpos.row).unwrap().len();

        if self.curr().cursorpos.col == curr_line_len {
            self.curr_mut().cursorpos.col = self.curr_mut().cursorpos.col.saturating_sub(1);
        }
        Ok(())
    }

    pub(crate) async fn backspace(&mut self) -> io::Result<Option<char>> {
        let prev_line_len = (self.curr_mut().cursorpos.row != 0).then(|| {
            let BufferTypeData::Regular { ref mut text, .. } = self.curr_mut().data.buffer_type
            else {
                todo!()
            };
            text.lines()
                .nth(self.curr_mut().cursorpos.row - 1)
                .unwrap()
                .len()
        });

        let BufferTypeData::Regular {
            ref mut text,
            id: curr_id,
            ..
        } = self.curr_mut().data.buffer_type
        else {
            todo!()
        };
        let (deleted, swaps) = text.client_mut(curr_id).backspace();
        if let Some(buffer::Socket { ref mut writer, .. }) = self.curr_mut().socket {
            writer.write_all(&C2S::Backspace(swaps).serialize()).await?;
        }

        if deleted.is_some() {
            if self.curr_mut().cursorpos.col == 0 {
                self.curr_mut().cursorpos.row -= 1;
                self.curr_mut().cursorpos.col = prev_line_len.unwrap();
            } else {
                self.curr_mut().cursorpos.col -= 1;
            }
        }

        Ok(deleted)
    }

    pub(crate) fn move_left(&mut self) {
        self.curr_mut().cursorpos.col = self.curr_mut().cursorpos.col.saturating_sub(1);
    }

    pub(crate) fn move_up(&mut self) {
        self.curr_mut().cursorpos.row = self.curr_mut().cursorpos.row.saturating_sub(1);
        self.curr_mut().cursorpos.col = cmp::min(self.curr_mut().cursorpos.col, {
            match &self.curr().data.buffer_type {
                BufferTypeData::Regular { text, .. } => text
                    .lines()
                    .nth(self.curr().cursorpos.row)
                    .map_or(0, |x| x.chars().count().saturating_sub(1)),
                BufferTypeData::Folder { inhabitants } => inhabitants
                    .get(self.curr().cursorpos.row)
                    .map_or(0, |x| x.name.len().saturating_sub(1)),
            }
        });
    }

    pub(crate) fn move_down(&mut self) {
        self.curr_mut().cursorpos.row = cmp::min(self.curr_mut().cursorpos.row + 1, {
            match &self.curr().data.buffer_type {
                BufferTypeData::Regular { text, .. } => text.lines().count().saturating_sub(1),
                BufferTypeData::Folder { inhabitants } => inhabitants.len().saturating_sub(1),
            }
        });
        self.curr_mut().cursorpos.col = cmp::min(self.curr_mut().cursorpos.col, {
            match &self.curr().data.buffer_type {
                BufferTypeData::Regular { text, .. } => text
                    .lines()
                    .nth(self.curr_mut().cursorpos.row)
                    .map_or(0, |x| x.chars().count().saturating_sub(2)),
                BufferTypeData::Folder { inhabitants } => inhabitants[self.curr().cursorpos.row]
                    .name
                    .len()
                    .saturating_sub(1),
            }
        });
    }

    pub(crate) fn move_right(&mut self) {
        self.curr_mut().cursorpos.col = cmp::min(self.curr_mut().cursorpos.col + 1, {
            match &self.curr().data.buffer_type {
                BufferTypeData::Regular { text, .. } => text
                    .lines()
                    .nth(self.curr().cursorpos.row)
                    .map_or(0, |x| x.chars().count().saturating_sub(1)),
                BufferTypeData::Folder { inhabitants } => inhabitants[self.curr().cursorpos.row]
                    .name
                    .len()
                    .saturating_sub(1),
            }
        });
    }

    /// Note this does not flush the writer
    pub(crate) async fn enter_insert(&mut self, pos: CursorPos) -> io::Result<()> {
        if !self.curr().data.modifiable {
            return Ok(());
        }
        let BufferTypeData::Regular {
            ref mut text,
            id: curr_id,
            ..
        } = self.curr_mut().data.buffer_type
        else {
            unreachable!()
        };
        let (_offset, _id) = text.client_mut(curr_id).enter_insert(pos);
        if let Some(buffer::Socket { ref mut writer, .. }) = self.curr_mut().socket {
            writer.write_all(&C2S::EnterInsert(pos).serialize()).await?;
        }
        self.modeinfo.set_mode(Mode::Insert);
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct ModeInfo {
    pub(crate) keymap: Vec<KeyEvent>,
    pub(crate) timer: Option<tokio::time::Sleep>,
    pub(crate) mode: Mode,
}

impl ModeInfo {
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }
}

/// Stores the current mode of the editor.
/// These work in the same way as vims modes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
    /// The cursor can move aronud
    Normal,
    /// Typing into buffers
    Insert,
    /// Writing a higher level command (: in (neo)vi(m))
    Command(String),
}

impl Default for Mode {
    fn default() -> Self {
        Self::Normal
    }
}
