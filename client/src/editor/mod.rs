//! This is the main module for handling editor stuff.
//! This includes handling keypressess and adding these
//! to the queue for sending to the server, but *not*
//! actually sending them
use core::str;
use std::cmp;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use text::Text;

#[derive(Debug)]
/// The main state for the entire editor. The entireity of the
/// view presented to the user can be rebuild from this
pub struct State {
    /// The rope stores the entire file being edited.
    pub text: Text,
    /// Our own id within the Text
    id: usize,
    /// Stores the current editing mode. This is
    /// effectively the same as Vims insert/Normal mode
    mode: Mode,
    /// stores where the cursor is located
    cursorpos: CursorPos,
}

/// `CursorPos` is effectively an (x, y) tuple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorPos {
    /// The row the cursor is on. This is effectively the line number
    pub row: usize,
    /// What column the cursor is on. Distance from the start of the line
    pub col: usize,
}

impl State {
    /// Creates a new appstate
    #[must_use]
    pub fn new(mut text: Text) -> Self {
        let id = text.add_client();
        Self {
            text,
            id,
            mode: Mode::Normal,
            cursorpos: CursorPos::default(),
        }
    }

    /// Returns an immutable reference to the internal
    /// cursors position
    #[must_use]
    pub const fn cursor(&self) -> &CursorPos {
        &self.cursorpos
    }

    /// Handles a keyevent. This method handles every `mode`
    pub fn handle_keyevent(&mut self, input: &KeyEvent) -> bool {
        match self.mode {
            Mode::Normal => self.handle_normal_keyevent(input),
            Mode::Insert => self.handle_insert_keyevent(input),
            Mode::Command(_) => self.handle_command_keyevent(input),
        }
    }

    /// handles a keypress as if were performed in `Insert` mode
    fn handle_insert_keyevent(&mut self, input: &KeyEvent) -> bool {
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
            KeyCode::Backspace => {
                todo!()
            }
            KeyCode::Left => self.cursorpos.col = self.cursorpos.col.saturating_sub(1),
            KeyCode::Right => {
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
            KeyCode::Up => {
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
            KeyCode::Down => {
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
            KeyCode::Enter => {
                todo!()
                // let cursor_pos = self.text.line_to_byte(self.cursorpos.row);
                // self.text.insert_char(cursor_pos + self.cursorpos.col, '\n');
                // self.cursorpos.row += 1;
                // self.cursorpos.col = 0;
            }
            KeyCode::Char(c) => {
                todo!()
                // let cursor_pos = self.text.line_to_byte(self.cursorpos.row);
                // self.text.insert_char(cursor_pos + self.cursorpos.col, c);
                // self.cursorpos.col += 1;
            }
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Home => todo!(),
            KeyCode::End => todo!(),
            KeyCode::PageUp => todo!(),
            KeyCode::PageDown => todo!(),
            KeyCode::Tab => todo!(),
            KeyCode::BackTab => todo!(),
            KeyCode::Delete => todo!(),
            KeyCode::Insert => todo!(),
            KeyCode::F(_) => todo!(),
            KeyCode::Null => todo!(),
            KeyCode::CapsLock => todo!(),
            KeyCode::ScrollLock => todo!(),
            KeyCode::NumLock => todo!(),
            KeyCode::PrintScreen => todo!(),
            KeyCode::Pause => todo!(),
            KeyCode::Menu => todo!(),
            KeyCode::KeypadBegin => todo!(),
            KeyCode::Media(_) => todo!(),
            KeyCode::Modifier(_) => todo!(),
        };
        false
    }

    /// handles a keypress as if were performed in `Normal` mode
    fn handle_normal_keyevent(&mut self, input: &KeyEvent) -> bool {
        match input.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('i') => {
                self.mode = Mode::Insert;
            }
            KeyCode::Char(':') => self.mode = Mode::Command(String::new()),
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
}
/// Stores the current mode of the editor.
/// These work in the same way as vims modes
#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Normal,
    Insert,
    Command(String),
}
