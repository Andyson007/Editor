//! This is the main module for handling editor stuff.
//! This includes handling keypressess and adding these
//! to the queue for sending to the server, but *not*
//! actually sending them
use core::panic;
use std::{
    cmp,
    io::{Read, Write},
    str,
};

use btep::{c2s::C2S, s2c::S2C};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use text::Text;
use tungstenite::WebSocket;
use utils::other::CursorPos;

#[derive(Debug)]
/// The main state for the entire editor. The entireity of the
/// view presented to the user can be rebuild from this
pub struct State<T> {
    /// The rope stores the entire file being edited.
    pub text: Text,
    /// Our own id within the Text
    id: usize,
    /// Stores the current editing mode. This is
    /// effectively the same as Vims insert/Normal mode
    mode: Mode,
    /// stores where the cursor is located
    cursorpos: CursorPos,
    socket: WebSocket<T>,
}

impl<T> State<T> {
    /// Creates a new appstate
    #[must_use]
    pub fn new(mut text: Text, socket: WebSocket<T>) -> Self {
        let id = text.add_client();
        Self {
            text,
            id,
            mode: Mode::Normal,
            cursorpos: CursorPos::default(),
            socket,
        }
    }

    /// Returns an immutable reference to the internal
    /// cursors position
    #[must_use]
    pub const fn cursor(&self) -> &CursorPos {
        &self.cursorpos
    }

    /// Handles a keyevent. This method handles every `mode`
    pub fn handle_keyevent(&mut self, input: &KeyEvent) -> bool
    where
        T: Read + Write,
    {
        match self.mode {
            Mode::Normal => self.handle_normal_keyevent(input),
            Mode::Insert => self.handle_insert_keyevent(input),
            Mode::Command(_) => self.handle_command_keyevent(input),
        }
    }

    /// handles a keypress as if were performed in `Insert` mode
    fn handle_insert_keyevent(&mut self, input: &KeyEvent) -> bool
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
            return true;
        }

        match input.code {
            KeyCode::Backspace => 'backspace: {
                if self.cursorpos == (CursorPos { row: 0, col: 0 }) {
                    break 'backspace;
                }
                if self.cursorpos.col == 0 {
                    self.cursorpos.row -= 1;
                    self.cursorpos.col = self.text.lines().nth(self.cursorpos.row).unwrap().len();
                } else {
                    self.cursorpos.col -= 1;
                }
                self.text.client(self.id).backspace();
                self.socket.write(C2S::Backspace.into()).unwrap();
                self.socket.flush().unwrap();
            }
            KeyCode::Enter => {
                self.text.client(self.id).push_char('\n');
                self.cursorpos.col = 0;
                self.cursorpos.row += 1;
                self.socket.write(C2S::Enter.into()).unwrap();
                self.socket.flush().unwrap();
            }
            KeyCode::Char(c) => {
                self.text.client(self.id).push_char(c);
                self.cursorpos.col += c.len_utf8();
                self.socket.write(C2S::Char(c).into()).unwrap();
                self.socket.flush().unwrap();
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
        false
    }

    /// handles a keypress as if were performed in `Normal` mode
    fn handle_normal_keyevent(&mut self, input: &KeyEvent) -> bool
    where
        T: Read + Write,
    {
        match input.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('i') => {
                self.enter_insert(self.cursorpos);
            }
            KeyCode::Char(':') => self.mode = Mode::Command(String::new()),
            KeyCode::Left | KeyCode::Char('h') => {
                self.cursorpos.col = self.cursorpos.col.saturating_sub(1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.cursorpos.col = cmp::min(
                    self.cursorpos.col + 1,
                    self.text
                        .lines()
                        .nth(self.cursorpos.row)
                        .unwrap()
                        .chars()
                        .count()
                        - 1,
                );
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.cursorpos.row = self.cursorpos.row.saturating_sub(1);
                self.cursorpos.col = cmp::min(
                    self.cursorpos.col,
                    self.text
                        .lines()
                        .nth(self.cursorpos.row)
                        .unwrap()
                        .chars()
                        .count()
                        .saturating_sub(1),
                );
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cursorpos.row =
                    cmp::min(self.cursorpos.row + 1, self.text.lines().count() - 1);
                self.cursorpos.col = cmp::min(
                    self.cursorpos.col,
                    self.text
                        .lines()
                        .nth(self.cursorpos.row)
                        .unwrap()
                        .chars()
                        .count()
                        .saturating_sub(2),
                );
            }
            _ => (),
        };
        false
    }

    fn handle_command_keyevent(&mut self, input: &KeyEvent) -> bool {
        let Mode::Command(ref mut cmd) = self.mode else {
            panic!("function incorrectly called");
        };
        match input.code {
            KeyCode::Backspace => drop(cmd.pop()),
            KeyCode::Enter => {
                let Mode::Command(ref cmd) = self.mode else {
                    panic!("function incorrectly called");
                };
                if self.execute_commad(cmd) {
                    return true;
                };
                self.mode = Mode::Normal;
            }
            KeyCode::Left => todo!(),
            KeyCode::Right => todo!(),
            KeyCode::Up => todo!(),
            KeyCode::Down => todo!(),
            KeyCode::Home => todo!(),
            KeyCode::End => todo!(),
            KeyCode::PageUp => todo!(),
            KeyCode::PageDown => todo!(),
            KeyCode::Tab => todo!(),
            KeyCode::BackTab => todo!(),
            KeyCode::Delete => todo!(),
            KeyCode::Insert => todo!(),
            KeyCode::F(_) => todo!(),
            KeyCode::Char(_) => todo!(),
            KeyCode::Null => todo!(),
            KeyCode::Esc => todo!(),
            KeyCode::CapsLock => todo!(),
            KeyCode::ScrollLock => todo!(),
            KeyCode::NumLock => todo!(),
            KeyCode::PrintScreen => todo!(),
            KeyCode::Pause => todo!(),
            KeyCode::Menu => todo!(),
            KeyCode::KeypadBegin => todo!(),
            KeyCode::Media(_) => todo!(),
            KeyCode::Modifier(_) => todo!(),
        }
        false
    }

    fn execute_commad(&self, cmd: &str) -> bool {
        cmd == "q"
    }

    fn enter_insert(&mut self, pos: CursorPos)
    where
        T: Read + Write,
    {
        let (_offset, _id) = self.text.client(self.id).enter_insert(pos);
        self.socket.write(C2S::EnterInsert(pos).into()).unwrap();
        self.socket.flush().unwrap();
        self.mode = Mode::Insert;
    }

    /// Fetches the network for any updates and updates the internal buffer accordingly
    /// # Panics
    /// the message received wasn't formatted properly
    pub fn update(&mut self)
    where
        T: Read + Write,
    {
        if let Ok(msg) = self.socket.read() {
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
                            client.backspace();
                            if client.data.as_ref().unwrap().pos.row == self.cursorpos.row
                                && client.data.as_ref().unwrap().pos.col < self.cursorpos.col
                            {
                                self.cursorpos.col -= 1;
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
                    }
                }
                S2C::NewClient => {
                    self.text.add_client();
                }
            }
        }
    }
}
/// Stores the current mode of the editor.
/// These work in the same way as vims modes
#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Normal,
    Insert,
    Command(String),
}
