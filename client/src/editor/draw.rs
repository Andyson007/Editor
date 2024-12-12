use crossterm::{
    cursor,
    style::{Color, Print, SetBackgroundColor},
    terminal::{self, ClearType},
};
use std::io;
use utils::other::CursorPos;

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
        let mut self_pos = None;
        let mut relative_col = 0;
        out.queue(cursor::MoveTo(0, 0))?;
        'outer: for buf in current_buffer.text.bufs() {
            let read_lock = buf.read();
            for c in read_lock.text.chars() {
                if c == '\n' {
                    relative_col = 0;
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
                    relative_col += 1;
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
                if occupied {
                    if buf == current_buffer.id {
                        self_pos = Some(CursorPos {
                            row: current_line - current_buffer.line_offset,
                            col: relative_col,
                        });
                    } else {
                        next_color = Some(
                            current_buffer.colors[if buf < current_buffer.id {
                                buf
                            } else {
                                buf - 1
                            }],
                        );
                    }
                }
            }
            if let Some((_, occupied)) = read_lock.buf {
                if occupied {
                    out.queue(SetBackgroundColor(Color::Reset))?;
                }
            }
        }
        if let Some(x) = next_color.take() {
            out.queue(SetBackgroundColor(x))?
                .queue(Print(' '))?
                .queue(SetBackgroundColor(Color::Reset))?;
        }
        if let Mode::Command(ref cmd) = self.mode {
            out.queue(cursor::MoveTo(0, size.1))?
                .queue(terminal::Clear(ClearType::CurrentLine))?
                .queue(Print(":"))?
                .queue(Print(cmd))?;
        } else {
            if let Some(ref info) = self.info {
                out.queue(cursor::MoveTo(size.0 - info.len() as u16, size.1))?
                    .queue(terminal::Clear(ClearType::CurrentLine))?
                    .queue(Print(info))?;
            }
            if let Some(CursorPos { row, col }) = self_pos {
                out.queue(cursor::MoveTo(
                    u16::try_from(col).unwrap(),
                    u16::try_from(row).unwrap(),
                ))?;
            } else {
                out.queue(cursor::MoveTo(
                    u16::try_from(current_buffer.cursor().col).unwrap(),
                    u16::try_from(current_buffer.cursor().row - current_buffer.line_offset)
                        .unwrap(),
                ))?;
            }
        }
        out.flush()?;
        Ok(())
    }
}
