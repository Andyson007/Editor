use core::str;
use std::cmp;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ropey::Rope;

#[derive(Clone, Debug)]
pub struct State {
    pub rope: Rope,
    mode: Mode,
    cursorpos: CursorPos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorPos {
    pub row: usize,
    pub col: usize,
}

impl State {
    #[must_use]
    pub const fn cursor(&self) -> &CursorPos {
        &self.cursorpos
    }

    pub fn new(data: &str) -> Result<Self, CreateState> {
        Ok(Self {
            rope: Rope::from_str(data),
            mode: Mode::Normal,
            cursorpos: CursorPos::default(),
        })
    }

    pub fn handle_keyevent(&mut self, input: &KeyEvent) -> bool {
        match self.mode {
            Mode::Normal => self.handle_normal_keyevent(input),
            Mode::Insert => self.handle_insert_keyevent(input),
            Mode::Command(_) => self.handle_command_keyevent(input),
        }
    }

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
            KeyCode::Backspace => 'backspace: {
                let del_pos = self.rope.line_to_byte(self.cursorpos.row) + self.cursorpos.col;
                if self.cursorpos == (CursorPos { row: 0, col: 0 }) {
                    break 'backspace;
                }
                if self.cursorpos.col == 0 {
                    self.cursorpos.row = self.cursorpos.row.saturating_sub(1);
                    self.cursorpos.col = self
                        .rope
                        .lines_at(self.cursorpos.row)
                        .next()
                        .unwrap()
                        .len_chars()
                        - 1;
                } else {
                    self.cursorpos.col -= 1;
                }
                if del_pos != 0 {
                    self.rope.remove((del_pos - 1)..del_pos);
                }
            }
            KeyCode::Left => self.cursorpos.col = self.cursorpos.col.saturating_sub(1),
            KeyCode::Right => {
                self.cursorpos.col = cmp::min(
                    self.cursorpos.col + 1,
                    self.rope
                        .lines_at(self.cursorpos.row)
                        .next()
                        .unwrap()
                        .len_chars()
                        - 1,
                );
            }
            KeyCode::Up => {
                self.cursorpos.row = self.cursorpos.row.saturating_sub(1);
                self.cursorpos.col = cmp::min(
                    self.cursorpos.col,
                    self.rope
                        .lines_at(self.cursorpos.row)
                        .next()
                        .unwrap()
                        .len_chars()
                        .saturating_sub(1),
                );
            }
            KeyCode::Down => {
                self.cursorpos.row = cmp::min(self.cursorpos.row + 1, self.rope.len_lines() - 1);
                self.cursorpos.col = cmp::min(
                    self.cursorpos.col,
                    self.rope
                        .lines_at(self.cursorpos.row)
                        .next()
                        .unwrap()
                        .len_chars()
                        .saturating_sub(2),
                );
            }
            KeyCode::Enter => {
                let cursor_pos = self.rope.line_to_byte(self.cursorpos.row);
                self.rope.insert_char(cursor_pos + self.cursorpos.col, '\n');
                self.cursorpos.row += 1;
                self.cursorpos.col = 0;
            }
            KeyCode::Char(c) => {
                let cursor_pos = self.rope.line_to_byte(self.cursorpos.row);
                self.rope.insert_char(cursor_pos + self.cursorpos.col, c);
                self.cursorpos.col += 1;
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

    fn handle_normal_keyevent(&mut self, input: &KeyEvent) -> bool {
        match input.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('i') => self.mode = Mode::Insert,
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

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum CreateState {
    BadFormat,
    BadUtf8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    Normal,
    Insert,
    Command(String),
}
