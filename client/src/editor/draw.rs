use btep::s2c::Inhabitant;
use crossterm::{
    cursor::{self, MoveToColumn, MoveToNextLine, RestorePosition, SavePosition},
    style::{Color, Print, SetBackgroundColor, SetForegroundColor},
    terminal::{self, ClearType},
};
use std::io;
use text::Text;
use utils::other::CursorPos;

use crossterm::QueueableCommand;

use super::{buffer::BufferTypeData, client::Mode, Client};

const PIPE_CHAR: char = '│';

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
        match &self.curr().data.buffer_type {
            BufferTypeData::Regular { text, colors, id } => {
                self.draw_regular(out, text, colors, *id)
            }
            BufferTypeData::Folder { inhabitants } => self.draw_inhabitants(out, inhabitants),
        }
    }

    fn draw_regular<E>(
        &self,
        out: &mut E,
        text: &Text,
        colors: &[Color],
        id: usize,
    ) -> io::Result<()>
    where
        E: QueueableCommand + io::Write,
    {
        let current_buffer = &self.buffers[self.current_buffer];

        out.queue(terminal::Clear(ClearType::All))?;
        let size = crossterm::terminal::size()?;
        let mut current_relative_line = 0;
        let mut next_color = None;
        let mut self_pos = None;
        let mut relative_col = 0;
        let mut cursor_offset = 0;
        out.queue(cursor::MoveTo(2, 0))?.queue(Print(PIPE_CHAR))?;
        'outer: for buf in text.bufs() {
            let read_lock = buf.read();
            for c in read_lock.text.chars() {
                if c == '\n' {
                    relative_col = 0;
                    if current_relative_line >= size.1 as usize + current_buffer.line_offset {
                        break 'outer;
                    };
                    if current_relative_line >= current_buffer.line_offset {
                        if let Some(x) = next_color.take() {
                            out.queue(SetBackgroundColor(x))?
                                .queue(Print(" "))?
                                .queue(MoveToNextLine(1))?
                                .queue(SetBackgroundColor(Color::Reset))?;
                        } else {
                            out.queue(MoveToNextLine(1))?;
                        }

                        out.queue(MoveToColumn(2))?.queue(Print(PIPE_CHAR))?;
                    }
                    current_relative_line += 1;
                } else if current_relative_line >= current_buffer.line_offset {
                    if relative_col >= size.0 as usize - 3 {
                        relative_col = 0;
                        current_relative_line += 1;
                        if current_buffer.cursor().row - current_buffer.line_offset
                            >= current_relative_line - cursor_offset
                        {
                            cursor_offset += 1;
                        }
                        out.queue(MoveToNextLine(1))?;

                        out.queue(MoveToColumn(2))?.queue(Print(PIPE_CHAR))?;
                    } else {
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
            }
            if let Some((buf, occupied)) = read_lock.buf {
                if occupied {
                    if buf == id {
                        self_pos = Some(CursorPos {
                            row: current_relative_line - current_buffer.line_offset,
                            col: relative_col,
                        });
                    } else {
                        let color = colors[if buf < id { buf } else { buf - 1 }];

                        let username = &text.client(buf).username;
                        out.queue(SavePosition)?
                            .queue(MoveToColumn(0))?
                            .queue(SetForegroundColor(color))?
                            .queue(Print(match username.len() {
                                ..2 => username,
                                2.. => &username[0..2],
                            }))?
                            .queue(SetForegroundColor(Color::Reset))?
                            .queue(RestorePosition)?;
                        next_color = Some(color);
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
        for _ in current_relative_line..size.1 as usize {
            out.queue(MoveToNextLine(1))?
                .queue(MoveToColumn(2))?
                .queue(Print(PIPE_CHAR))?;
        }
        if let Mode::Command(ref cmd) = self.modeinfo.mode {
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
                    u16::try_from(col).unwrap() + 3,
                    u16::try_from(row).unwrap(),
                ))?;
            } else {
                out.queue(cursor::MoveTo(
                    u16::try_from(current_buffer.cursor().col + 3).unwrap(),
                    u16::try_from(
                        current_buffer.cursor().row - current_buffer.line_offset + cursor_offset,
                    )
                    .unwrap(),
                ))?;
            }
        }
        out.flush()?;
        Ok(())
    }

    fn draw_inhabitants<E>(&self, out: &mut E, inhabitants: &[Inhabitant]) -> io::Result<()>
    where
        E: QueueableCommand + io::Write,
    {
        out.queue(terminal::Clear(ClearType::All))?;
        out.queue(cursor::MoveTo(0, 0))?;
        for inhabitant in inhabitants.iter().skip(self.curr().line_offset) {
            if inhabitant.is_folder {
                out.queue(SetForegroundColor(Color::DarkBlue))?;
            }
            out.queue(Print(inhabitant.name.to_str().unwrap()))?;
            if inhabitant.is_folder {
            out.queue(Print('/'))?;
                out.queue(SetForegroundColor(Color::Reset))?;
            }
            out.queue(cursor::MoveToNextLine(1))?;
        }

        out.queue(cursor::MoveTo(
            self.curr().cursorpos.col as u16,
            self.curr().cursorpos.row as u16,
        ))?;

        out.flush()?;
        Ok(())
    }
}
