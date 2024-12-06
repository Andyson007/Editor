//! This is the main module for handling editor stuff.
//! This includes handling keypressess and adding these
//! to the queue for sending to the server, but *not*
//! actually sending them

use std::{
    cmp,
    io::{BufReader, Read, Write},
    mem,
};

use btep::c2s::C2S;
use buffer::Buffer;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use text::Text;
use tungstenite::WebSocket;
use utils::other::CursorPos;
mod buffer;
mod draw;

#[derive(Default)]
pub struct Client<T> {
    buffers: Vec<Buffer<T>>,
    current_buffer: usize,
    /// Stores the current editing mode. This is
    /// effectively the same as Vims insert/Normal mode
    pub(crate) mode: Mode,
}

impl<T> Client<T> {
    pub fn new() -> Self {
        let buf = Buffer::<T>::new(Text::new(), None);
        Self {
            buffers: Vec::from([buf]),
            current_buffer: 0,
            mode: Mode::Normal,
        }
    }

    pub fn new_with_buffer(text: Text, socket: Option<WebSocket<T>>) -> Self {
        let buf = Buffer::<T>::new(text, socket);
        Self {
            buffers: Vec::from([buf]),
            current_buffer: 0,
            mode: Mode::Normal,
        }
    }

    pub fn curr(&mut self) -> &mut Buffer<T> {
        &mut self.buffers[self.current_buffer]
    }

    fn execute_commad(&mut self, cmd: &str) -> bool
    where
        T: Read + Write,
    {
        match cmd {
            "q" => return self.close_current_buffer(),
            "w" => self.curr().save(),
            "help" => self.add_buffer(Text::original_from_str(include_str!("../../../help")), None),
            _ => (),
        }
        false
    }

    fn add_buffer(&mut self, text: Text, socket: Option<WebSocket<T>>) {
        self.buffers.push(Buffer::new(text, socket));
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

    /// Handles a keyevent. This method handles every `mode`
    pub fn handle_keyevent(&mut self, input: &KeyEvent) -> bool
    where
        T: Read + Write,
    {
        match self.mode {
            Mode::Normal => self.handle_normal_keyevent(input),
            Mode::Insert => self.handle_insert_keyevent(input),
            Mode::Command(_) => {
                if self.handle_command_keyevent(input) {
                    return true;
                }
            }
        };
        false
    }
    fn take_mode(&mut self) -> Option<String> {
        if let Mode::Command(cmd) = mem::replace(&mut self.mode, Mode::Normal) {
            Some(cmd)
        } else {
            None
        }
    }

    fn handle_command_keyevent(&mut self, input: &KeyEvent) -> bool
    where
        T: Read + Write,
    {
        let Mode::Command(ref mut cmd) = self.mode else {
            panic!("function incorrectly called");
        };
        match input.code {
            KeyCode::Backspace => drop(cmd.pop()),
            KeyCode::Enter => {
                let cmd = self.take_mode().unwrap();
                if self.execute_commad(&cmd) {
                    return true;
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Char(c) => cmd.push(c),
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Tab
            | KeyCode::BackTab
            | KeyCode::Delete
            | KeyCode::Insert
            | KeyCode::F(_)
            | KeyCode::Null
            | KeyCode::CapsLock
            | KeyCode::ScrollLock
            | KeyCode::NumLock
            | KeyCode::PrintScreen
            | KeyCode::Pause
            | KeyCode::Menu
            | KeyCode::KeypadBegin
            | KeyCode::Media(_)
            | KeyCode::Modifier(_) => todo!("Todo in command"),
        }
        false
    }

    /// handles a keypress as if were performed in `Insert` mode
    fn handle_insert_keyevent(&mut self, input: &KeyEvent)
    where
        T: Read + Write,
    {
        if matches!(
            input,
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                kind: KeyEventKind::Press,
                ..
            }
        ) {
            self.mode = Mode::Insert;
            return;
        }

        let curr_id = self.curr().id;

        match input.code {
            KeyCode::Backspace => 'backspace: {
                if self.curr().cursorpos == (CursorPos { row: 0, col: 0 }) {
                    break 'backspace;
                }
                if self.curr().cursorpos.col == 0 {
                    self.curr().cursorpos.row -= 1;
                    self.curr().cursorpos.col = self
                        .curr()
                        .text
                        .lines()
                        .nth(self.curr().cursorpos.row)
                        .unwrap()
                        .len();
                } else {
                    self.curr().cursorpos.col -= 1;
                }
                self.curr().text.client(curr_id).backspace();
                if let Some(ref mut socket) = self.curr().socket {
                    socket.write(C2S::Backspace.into()).unwrap();
                    socket.flush().unwrap();
                }
            }
            KeyCode::Enter => {
                self.curr().text.client(curr_id).push_char('\n');
                self.curr().cursorpos.col = 0;
                self.curr().cursorpos.row += 1;
                if let Some(ref mut socket) = self.curr().socket {
                    socket.write(C2S::Enter.into()).unwrap();
                    socket.flush().unwrap();
                }
            }
            KeyCode::Char(c) => {
                self.curr().text.client(curr_id).push_char(c);
                self.curr().cursorpos.col += c.len_utf8();
                if let Some(ref mut socket) = self.curr().socket {
                    socket.write(C2S::Char(c).into()).unwrap();
                    socket.flush().unwrap();
                }
            }
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Home => todo!(),
            KeyCode::End => todo!(),
            KeyCode::PageUp | KeyCode::PageDown => todo!(),
            KeyCode::Tab | KeyCode::BackTab => todo!(),
            KeyCode::Delete => todo!(),
            KeyCode::Insert => todo!(),
            KeyCode::F(_) => todo!(),
            KeyCode::Null => todo!(),
            KeyCode::CapsLock => todo!(),
            KeyCode::ScrollLock
            | KeyCode::NumLock
            | KeyCode::PrintScreen
            | KeyCode::Pause
            | KeyCode::Menu
            | KeyCode::KeypadBegin
            | KeyCode::Media(_) => todo!(),
            KeyCode::Modifier(_) => todo!(),
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => (),
        };
    }

    /// handles a keypress as if were performed in `Normal` mode
    fn handle_normal_keyevent(&mut self, input: &KeyEvent)
    where
        T: Read + Write,
    {
        match input.code {
            KeyCode::Char('i') => {
                let pos = self.curr().cursorpos;
                self.enter_insert(pos);
            }
            KeyCode::Char(':') => self.mode = Mode::Command(String::new()),
            KeyCode::Left | KeyCode::Char('h') => {
                self.curr().cursorpos.col = self.curr().cursorpos.col.saturating_sub(1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.curr().cursorpos.col = cmp::min(
                    self.curr().cursorpos.col + 1,
                    self.curr()
                        .text
                        .lines()
                        .nth(self.curr().cursorpos.row)
                        .unwrap()
                        .chars()
                        .count()
                        - 1,
                );
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.curr().cursorpos.row = self.curr().cursorpos.row.saturating_sub(1);
                self.curr().cursorpos.col = cmp::min(
                    self.curr().cursorpos.col,
                    self.curr()
                        .text
                        .lines()
                        .nth(self.curr().cursorpos.row)
                        .unwrap()
                        .chars()
                        .count()
                        .saturating_sub(1),
                );
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.curr().cursorpos.row = cmp::min(
                    self.curr().cursorpos.row + 1,
                    self.curr().text.lines().count() - 1,
                );
                self.curr().cursorpos.col = cmp::min(
                    self.curr().cursorpos.col,
                    self.curr()
                        .text
                        .lines()
                        .nth(self.curr().cursorpos.row)
                        .unwrap()
                        .chars()
                        .count()
                        .saturating_sub(2),
                );
            }
            _ => (),
        };
    }

    fn enter_insert(&mut self, pos: CursorPos)
    where
        T: Read + Write,
    {
        let curr_id = self.curr().id;
        let (_offset, _id) = self.curr().text.client(curr_id).enter_insert(pos);
        if let Some(ref mut socket) = self.curr().socket {
            socket.write(C2S::EnterInsert(pos).into()).unwrap();
            socket.flush().unwrap();
        }
        self.mode = Mode::Insert;
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

impl Default for Mode {
    fn default() -> Self {
        Self::Normal
    }
}
