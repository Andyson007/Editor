//! A client side for my server side collaboration thing
pub mod editor;
pub mod errors;

use base64::{prelude::BASE64_STANDARD, Engine};
use btep::prelude::S2C;
use crossterm::{
    cursor,
    event::{self, EnableBracketedPaste, Event},
    execute,
    style::Print,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, size, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
    ExecutableCommand, QueueableCommand,
};
use editor::{Mode, State};
use std::{
    io::{self, Write},
    net::{SocketAddrV4, TcpStream},
    str,
    time::Duration,
};
use text::Text;
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
pub fn run(
    address: SocketAddrV4,
    username: &str,
    password: Option<&str>,
) -> color_eyre::Result<()> {
    let mut out = io::stdout();
    errors::install_hooks()?;

    let (mut socket, _response) = connect_with_auth(address, username, password);

    let message = loop {
        if let Ok(x) = socket.read() {
            break x;
        }
    };

    let S2C::Full(initial_text) = S2C::<Text>::from_message(message).unwrap() else {
        panic!("Initial message in wrong protocol")
    };

    execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
    enable_raw_mode().unwrap();

    let mut app = State::new(initial_text, socket);

    redraw(&mut out, 0, &app)?;

    out.execute(cursor::MoveTo(0, 0)).unwrap();

    loop {
        // `read()` blocks until an `Event` is available
        if event::poll(Duration::from_secs(0)).unwrap() {
            match event::read()? {
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
        } else {
            app.update();
        }
        redraw(&mut out, 0, &app)?;
        out.flush()?;
    }

    disable_raw_mode().unwrap();
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn redraw<E, T>(out: &mut E, startline: usize, state: &State<T>) -> io::Result<()>
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
    let size = crossterm::terminal::size()?;
    if let Mode::Command(ref cmd) = state.mode {
        out.queue(cursor::MoveTo(0, size.1))?
            .queue(Print(":"))?
            .queue(Print(cmd))?
            .queue(Print(
                std::iter::repeat_n(' ', usize::from(size.0) - 1 - cmd.len()).collect::<String>(),
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
    address: SocketAddrV4,
    username: &str,
    password: Option<&str>,
) -> (
    WebSocket<MaybeTlsStream<TcpStream>>,
    http::Response<Option<Vec<u8>>>,
) {
    let uri = Uri::builder()
        .scheme("ws")
        .authority(address.to_string())
        .path_and_query("/")
        .build()
        .unwrap();
    let authority = uri.authority().unwrap().as_str();
    let host = authority
        .find('@')
        .map_or_else(|| authority, |idx| authority.split_at(idx + 1).1);

    assert!(!host.is_empty());

    let request = Request::builder()
        .method("GET")
        .header("Host", host)
        .header("Connection", "Upgrade")
        .header(
            "Authorization",
            format!(
                "Basic {}",
                BASE64_STANDARD.encode(format!("{username}:{}", password.unwrap_or("")))
            ),
        )
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", generate_key())
        .uri(uri)
        .body(())
        .unwrap();
    let mut ret = connect(request).unwrap();
    match ret.0.get_mut() {
        MaybeTlsStream::Plain(x) => x.set_nonblocking(true).unwrap(),
        _ => unreachable!(),
    };
    ret
}
