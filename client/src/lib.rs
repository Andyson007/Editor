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
use text::Text;

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

/// Runs a the client side of the editor
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::missing_errors_doc)]
pub fn run() -> color_eyre::Result<()> {
    let mut out = io::stdout();
    errors::install_hooks()?;

    execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
    enable_raw_mode().unwrap();

    let (mut socket, _response) = connect_with_auth("ws://localhost:3012");

    let Btep::Full(initial_text) = Btep::<Text>::from_message(socket.read()?) else {
        panic!("Initial message in wrong protocol")
    };

    let mut app = State::new(initial_text);

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
        out.queue(cursor::MoveTo(0, u16::try_from(linenr).unwrap()))?
            .queue(Print(
                line.chars().take(size()?.0.into()).collect::<String>(),
            ))?;
    }
    out.queue(cursor::MoveTo(
        u16::try_from(state.cursor().col).unwrap(),
        u16::try_from(state.cursor().row).unwrap(),
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
        .header("Authorization", "Basic YTpi")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", generate_key())
        .uri(uri)
        .body(())
        .unwrap();
    connect(req).unwrap()
}
