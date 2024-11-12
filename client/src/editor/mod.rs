use core::str;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ropey::Rope;
use tungstenite::Message;

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
    pub fn cursor(&self) -> &CursorPos {
        &self.cursorpos
    }

    pub fn new(data: Message) -> Result<Self, CreateState> {
        let Message::Binary(data) = data else {
            return Err(CreateState::BadFormat);
        };
        Ok(Self {
            rope: Rope::from_str(
                str::from_utf8(data.as_slice()).map_err(|_| CreateState::BadUtf8)?,
            ),
            mode: Mode::Normal,
            cursorpos: CursorPos::default(),
        })
    }

    pub fn handle_keyevent(&mut self, input: &KeyEvent) -> bool {
        match self.mode {
            Mode::Normal => self.handle_normal_keyevent(input),
            Mode::Insert => self.handle_insert_keyevent(input),
            Mode::Command => self.handle_command_keyevent(input),
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
            KeyCode::Backspace => {
                let del_pos = self.rope.line_to_byte(self.cursorpos.row) + self.cursorpos.col;
                if del_pos != 0 {
                    self.rope.remove((del_pos - 1)..del_pos);
                    self.cursorpos.col -= 1;
                }
            }
            KeyCode::Enter => {
                let cursor_pos = self.rope.line_to_byte(self.cursorpos.row);
                self.rope.insert_char(cursor_pos + self.cursorpos.col, '\n');
                self.cursorpos.row += 1;
                self.cursorpos.col = 0;
            }
            KeyCode::Left => self.cursorpos.col = self.cursorpos.col.saturating_sub(1),
            KeyCode::Right => self.cursorpos.col += 1,
            KeyCode::Up => self.cursorpos.row = self.cursorpos.row.saturating_sub(1),
            KeyCode::Down => self.cursorpos.row += 1,
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
            KeyCode::Esc => self.mode = Mode::Normal,
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
            _ => (),
        };
        false
    }

    fn handle_command_keyevent(&mut self, input: &KeyEvent) -> bool {
        false
    }
}

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum CreateState {
    BadFormat,
    BadUtf8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    Insert,
    Command,
}
