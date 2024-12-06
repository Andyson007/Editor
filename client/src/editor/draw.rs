use crossterm::{
    cursor,
    style::Print,
    terminal::{self, ClearType},
};
use std::io;

use crossterm::QueueableCommand;

use super::{Client, Mode};

impl<T> Client<T> {
    pub fn redraw<E>(&self, out: &mut E) -> io::Result<()>
    where
        E: QueueableCommand + io::Write,
    {
        let buffer = &self.buffers[self.current_buffer];

        out.queue(terminal::Clear(ClearType::All))?;
        for (linenr, line) in buffer
            .text
            .lines()
            .skip(buffer.line_offset)
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
        if let Mode::Command(ref cmd) = buffer.mode {
            out.queue(cursor::MoveTo(0, size.1))?
                .queue(Print(":"))?
                .queue(Print(cmd))?
                .queue(Print(
                    std::iter::repeat_n(' ', usize::from(size.0) - 1 - cmd.len())
                        .collect::<String>(),
                ))?;
        }
        out.queue(cursor::MoveTo(
            u16::try_from(buffer.cursor().col).unwrap(),
            u16::try_from(buffer.cursor().row - buffer.line_offset).unwrap(),
        ))?;
        out.flush()?;
        Ok(())
    }
}
