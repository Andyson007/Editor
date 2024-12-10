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
        let cursors: HashMap<(usize, usize), Color> = self
            .curr()
            .text
            .clients()
            .iter()
            .filter(|client| client.bufnr != self.curr().id)
            .zip(self.curr().colors.iter())
            .filter_map(|(t, c)| t.data.as_ref().map(|x| (x.pos.into(), *c)))
            .collect();

        let buffer = &self.buffers[self.current_buffer];

        out.queue(terminal::Clear(ClearType::All))?;
        for (linenr, line) in buffer
            .text
            .lines()
            .skip(buffer.line_offset)
            .take(terminal::size()?.1.into())
            .enumerate()
        {
            out.queue(cursor::MoveTo(0, u16::try_from(linenr).unwrap()))?;
            for (colnr, c) in line
                .chars()
                .chain([' '])
                .take(terminal::size()?.0.into())
                .enumerate()
            {
                if let Some(color) = cursors.get(&(linenr, colnr)) {
                    out.queue(SetBackgroundColor(*color))?
                        .queue(SetForegroundColor(Color::DarkGrey))?;
                }
                out.queue(Print(c))?;
                if cursors.contains_key(&(linenr, colnr)) {
                    out.queue(SetBackgroundColor(Color::Reset))?
                        .queue(SetForegroundColor(Color::Reset))?;
                }
            }
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
            u16::try_from(buffer.cursor().col).unwrap(),
            u16::try_from(buffer.cursor().row - buffer.line_offset).unwrap(),
        ))?;
        out.flush()?;
        Ok(())
    }
}
