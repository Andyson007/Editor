use crossterm::{
    cursor,
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType},
};
use std::{collections::HashMap, io};

use crossterm::QueueableCommand;

use super::{Client, Mode};

impl Client {
    /// draws the current client to the screen
    /// # Errors
    /// - failing to write to the terminal
    /// # Panics
    /// - faliing to convert from a usize to a u16
    pub fn redraw<E>(&self, out: &mut E) -> io::Result<()>
    where
        E: QueueableCommand + io::Write,
    {
        let current_buffer = &self.buffers[self.current_buffer];

        out.queue(terminal::Clear(ClearType::All))?;
        let size = crossterm::terminal::size()?;
        let mut current_line = 0;
        out.queue(cursor::MoveTo(0, 0))?;
        'outer: for buf in current_buffer.text.bufs() {
            for c in buf.read().text.chars() {
                if c == '\n' {
                    if current_line >= size.1 as usize + current_buffer.line_offset - 1 {
                        break 'outer;
                    };
                    if current_line >= current_buffer.line_offset {
                        out.queue(Print("\r\n"))?;
                    }
                    current_line += 1;
                } else if current_line + 1 >= current_buffer.line_offset {
                    out.queue(Print(c))?;
                }
            }
        }
        if let Mode::Command(ref cmd) = self.mode {
            out.queue(cursor::MoveTo(0, size.1))?
                .queue(terminal::Clear(ClearType::CurrentLine))?
                .queue(Print(":"))?
                .queue(Print(cmd))?;
        }
        out.queue(cursor::MoveTo(
            u16::try_from(current_buffer.cursor().col).unwrap(),
            u16::try_from(current_buffer.cursor().row - current_buffer.line_offset).unwrap(),
        ))?;
        out.flush()?;
        Ok(())
    }
}
