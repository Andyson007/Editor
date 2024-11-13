//! A client for my server side md editing thing
use crossterm::{
    cursor,
    event::{read, EnableBracketedPaste, Event},
    execute,
    style::Print,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, size, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    ExecutableCommand, QueueableCommand,
};

use core::str;
use std::io::{self, Write};
use tungstenite::{connect, Message};

use client::{editor::State, errors};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    let mut out = io::stdout();
    errors::install_hooks()?;

    execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
    enable_raw_mode().unwrap();

    let (mut socket, _response) = connect("ws://localhost:3012").unwrap();
    let Message::Binary(data) = socket.read()? else {
        panic!("Couldn't read binary stream from socket");
    };
    let mut app = State::new(str::from_utf8(&data).unwrap());

    redraw(&mut out, 0, &app)?;

    out.execute(cursor::MoveTo(0, 0)).unwrap();

    loop {
        // `read()` blocks until an `Event` is available
        match read()? {
            Event::Key(event) => {
                if app.handle_keyevent(&event) {
                    break;
                };
            }
            Event::Mouse(_event) => todo!("No mouse support sorry"),
            Event::Paste(_data) => todo!("No paste support sorry"),
            Event::Resize(_width, _height) => (),
            Event::FocusGained | Event::FocusLost => (),
        };
        redraw(&mut out, 0, &app)?;
        out.flush()?;
    }

    disable_raw_mode().unwrap();
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn redraw<E>(out: &mut E, startline: usize, state: &State) -> io::Result<()>
where
    E: QueueableCommand + io::Write,
{
    out.queue(terminal::Clear(ClearType::All))?;
    for (linenr, line) in state
        .rope
        .lines_at(startline)
        .take(size()?.1.into())
        .enumerate()
    {
        out.queue(cursor::MoveTo(0, linenr as u16))?.queue(Print(
            line.chars().take(size()?.0.into()).collect::<String>(),
        ))?;
    }
    out.queue(cursor::MoveTo(
        state.cursor().col as u16,
        state.cursor().row as u16,
    ))?;
    out.flush()?;
    Ok(())
}
