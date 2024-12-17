use crate::editor::buffer;
use std::{cmp, io};
use tokio::{io::AsyncWriteExt, net::TcpStream};

use btep::{c2s::C2S, Serialize};
use crossterm::{event::KeyEvent, style::Color};
use text::Text;
use utils::other::CursorPos;

use super::buffer::Buffer;
/// Represents a single client.
#[derive(Default, Debug)]
pub struct Client {
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
    #[must_use]
    pub fn new(username: String) -> Self {
        Self::new_with_buffer(username, Text::new(), Vec::new(), None)
    }

    /// Cretaes a new client with a prepopulated text buffer
    #[must_use]
    pub fn new_with_buffer(
        username: String,
        text: Text,
        colors: Vec<Color>,
        socket: Option<TcpStream>,
    ) -> Self {
        let buf = Buffer::new(username, text, colors, socket);
        Self {
            buffers: Vec::from([buf]),
            current_buffer: 0,
            modeinfo: ModeInfo::default(),
            info: Some("Press Escape then :help to view help".to_string()),
        }
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
            ),
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
    ) {
        self.buffers
            .push(Buffer::new(username, text, colors, socket));
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
        let curr_id = self.curr().id;
        self.curr_mut().text.client_mut(curr_id).push_char(c);
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
        let curr_id = self.curr().id;
        self.modeinfo.set_mode(Mode::Normal);
        self.curr_mut().text.client_mut(curr_id).exit_insert();

        if let Some(buffer::Socket { ref mut writer, .. }) = self.curr_mut().socket {
            writer.write_all(&C2S::ExitInsert.serialize()).await?
        }
        let curr_line_len = self
            .curr()
            .text
            .lines()
            .nth(self.curr().cursorpos.row)
            .unwrap()
            .len();

        if self.curr().cursorpos.col == curr_line_len {
            self.curr_mut().cursorpos.col -= 1;
        }
        Ok(())
    }

    pub(crate) async fn backspace(&mut self) -> io::Result<Option<char>> {
        let curr_id = self.curr().id;

        let prev_line_len = (self.curr_mut().cursorpos.row != 0).then(|| {
            self.curr_mut()
                .text
                .lines()
                .nth(self.curr_mut().cursorpos.row - 1)
                .unwrap()
                .len()
        });

        let (deleted, swaps) = self.curr_mut().text.client_mut(curr_id).backspace();
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
        self.curr_mut().cursorpos.col = cmp::min(
            self.curr_mut().cursorpos.col,
            self.curr_mut()
                .text
                .lines()
                .nth(self.curr_mut().cursorpos.row)
                .map_or(0, |x| x.chars().count().saturating_sub(1)),
        );
    }

    pub(crate) fn move_down(&mut self) {
        self.curr_mut().cursorpos.row = cmp::min(
            self.curr_mut().cursorpos.row + 1,
            self.curr_mut().text.lines().count().saturating_sub(1),
        );
        self.curr_mut().cursorpos.col = cmp::min(
            self.curr_mut().cursorpos.col,
            self.curr_mut()
                .text
                .lines()
                .nth(self.curr_mut().cursorpos.row)
                .map_or(0, |x| x.chars().count().saturating_sub(2)),
        );
    }

    pub(crate) fn move_right(&mut self) {
        self.curr_mut().cursorpos.col = cmp::min(
            self.curr_mut().cursorpos.col + 1,
            self.curr_mut()
                .text
                .lines()
                .nth(self.curr_mut().cursorpos.row)
                .map_or(0, |x| x.chars().count().saturating_sub(1)),
        );
    }

    /// Note this does not flush the writer
    pub(crate) async fn enter_insert(&mut self, pos: CursorPos) -> io::Result<()> {
        let curr_id = self.curr_mut().id;
        let (_offset, _id) = self.curr_mut().text.client_mut(curr_id).enter_insert(pos);
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
