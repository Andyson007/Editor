//! A client side for my server side collaboration thing
pub mod editor;
pub mod errors;

use base64::{prelude::BASE64_STANDARD, Engine};
use btep::prelude::S2C;
use crossterm::{
    cursor,
    event::{self, EnableBracketedPaste, Event},
    execute,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use editor::{Client, Mode};
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

    let mut app = Client::new_with_buffer(initial_text, Some(socket));

    app.redraw(&mut out)?;

    out.execute(cursor::MoveTo(0, 0)).unwrap();

    loop {
        if if event::poll(Duration::from_secs(0)).unwrap() {
            match event::read()? {
                Event::Key(event) => {
                    if app.curr().handle_keyevent(&event) {
                        break;
                    };
                }
                Event::Mouse(_event) => todo!("No mouse support sorry"),
                Event::Paste(_data) => todo!("No paste support sorry"),
                Event::Resize(_width, _height) => (),
                Event::FocusGained | Event::FocusLost => (),
            };
            true
        } else {
            app.curr().update()
        } {
            app.curr().recalculate_cursor(terminal::size()?);
            app.redraw(&mut out).unwrap();
            out.flush()?;
        }
    }

    disable_raw_mode().unwrap();
    execute!(io::stdout(), LeaveAlternateScreen)?;
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
