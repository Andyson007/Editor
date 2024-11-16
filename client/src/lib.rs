//! A client side for my server side collaboration thing
pub mod editor;
pub mod errors;

use btep::Btep;
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
use piece_table::Piece;

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
    WebSocket,
};

pub fn run() -> color_eyre::Result<()> {
    let mut out = io::stdout();
    errors::install_hooks()?;

    execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
    enable_raw_mode().unwrap();

    let (mut socket, _response) = connect_with_auth("ws://localhost:3012");

    let Btep::Full(initial_file) = Btep::<Piece>::from_message(socket.read()?) else {
        panic!("Initial message in wrong protocol")
    };

    let mut app = State::new(initial_file);

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
        .text
        .lines()
        .skip(startline)
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
        .map_or_else(|| authority, |idx| authority.split_at(idx + 1).1);

    assert!(!host.is_empty());

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
