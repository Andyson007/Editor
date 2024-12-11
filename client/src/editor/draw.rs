use crossterm::{
    cursor,
    style::{self, Color, Print, SetBackgroundColor, SetForegroundColor},
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
        let mut next_color = None;
        out.queue(cursor::MoveTo(0, 0))?;
        'outer: for buf in current_buffer.text.bufs() {
            let read_lock = buf.read();
            for c in read_lock.text.chars() {
                if c == '\n' {
                    if current_line >= size.1 as usize + current_buffer.line_offset - 1 {
                        break 'outer;
                    };
                    if current_line >= current_buffer.line_offset {
                        if let Some(x) = next_color.take() {
                            out.queue(SetBackgroundColor(x))?
                                .queue(Print(" \r\n"))?
                                .queue(SetBackgroundColor(Color::Reset))?;
                        } else {
                            out.queue(Print("\r\n"))?;
                        }
                    }
                    current_line += 1;
                } else if current_line + 1 >= current_buffer.line_offset {
                    if let Some(x) = next_color.take() {
                        out.queue(SetBackgroundColor(x))?
                            .queue(Print(c))?
                            .queue(SetBackgroundColor(Color::Reset))?;
                    } else {
                        out.queue(Print(c))?;
                    }
                }
            }
            if let Some((buf, occupied)) = read_lock.buf {
                if occupied && buf != current_buffer.id {
                    next_color = Some(
                        current_buffer.colors[if buf < current_buffer.id {
                            buf
                        } else {
                            buf - 1
                        }],
                    );
                }
            }
            if let Some((_, occupied)) = read_lock.buf {
                if occupied {
                    out.queue(SetBackgroundColor(Color::Reset))?;
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
