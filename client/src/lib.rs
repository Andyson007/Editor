//! A client side for my server side collaboration thing
pub mod editor;
pub mod errors;

use btep::{Btep, FromMessage};
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
use editor::State;

use core::str;
use std::{
    io::{self, Write},
    net::TcpStream,
    str::FromStr,
};
use tungstenite::{
    connect,
    handshake::client::{generate_key, Request},
    http::{self, Uri},
    stream::MaybeTlsStream,
    Message, WebSocket,
};

pub fn run() -> color_eyre::Result<()> {
    let mut out = io::stdout();
    errors::install_hooks()?;

    execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
    enable_raw_mode().unwrap();

    let (mut socket, _response) = connect_with_auth("ws://localhost:3012");

    let initial_file = <Btep<Box<[u8]>> as FromMessage>::from_message(socket.read()?);

    let mut app = State::new(initial_file.as_utf8());

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

fn connect_with_auth(
    uri: &str,
) -> (
    WebSocket<MaybeTlsStream<TcpStream>>,
    http::Response<Option<Vec<u8>>>,
) {
    let uri = Uri::from_str(uri).unwrap();
    let authority = uri.authority().unwrap().as_str();
    let host = authority
        .find('@')
        .map(|idx| authority.split_at(idx + 1).1)
        .unwrap_or_else(|| authority);

    if host.is_empty() {
        panic!("No hostname")
    }

    let req = Request::builder()
        .method("GET")
        .header("Host", host)
        .header("Connection", "Upgrade")
        .header("test", "test")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", generate_key())
        .uri(uri)
        .body(())
        .unwrap();
    connect(req).unwrap()
}
