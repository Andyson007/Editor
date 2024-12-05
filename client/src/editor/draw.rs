use core::panic;
use crossterm::{
    cursor,
    style::Print,
    terminal::{self, ClearType},
};
use std::io;

use crossterm::QueueableCommand;

use super::{Mode, State};

impl<T> State<T> {
    pub fn redraw<E>(&self, out: &mut E) -> io::Result<()>
    where
        E: QueueableCommand + io::Write,
    {
        out.queue(terminal::Clear(ClearType::All))?;
        for (linenr, line) in self
            .text
            .lines()
            .skip(self.line_offset)
            .take(terminal::size()?.1.into())
            .enumerate()
        {
            out.queue(cursor::MoveTo(0, u16::try_from(linenr).unwrap()))?
                .queue(Print(
                    line.chars()
                        .take(terminal::size()?.0.into())
                        .collect::<String>(),
                ))?;
        }
        let size = crossterm::terminal::size()?;
        if let Mode::Command(ref cmd) = self.mode {
            out.queue(cursor::MoveTo(0, size.1))?
                .queue(Print(":"))?
                .queue(Print(cmd))?
                .queue(Print(
                    std::iter::repeat_n(' ', usize::from(size.0) - 1 - cmd.len())
                        .collect::<String>(),
                ))?;
        }
        out.queue(cursor::MoveTo(
            u16::try_from(self.cursor().col).unwrap(),
            u16::try_from(self.cursor().row - self.line_offset).unwrap(),
        ))?;
        out.flush()?;
        Ok(())
    }

    pub fn recalculate_cursor(&mut self, (cols, rows): (u16, u16)) {
        if self.line_offset > self.cursorpos.row {
            self.line_offset = self.cursorpos.row;
        } else if self.line_offset + usize::from(rows) <= self.cursorpos.row {
            self.line_offset = self.cursorpos.row - usize::from(rows) + 1;
        }
    }
}
