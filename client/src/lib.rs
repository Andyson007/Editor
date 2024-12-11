//! A client side for my server side collaboration thing
pub mod editor;
pub mod errors;

use btep::{prelude::S2C, Deserialize};
use crossterm::{
    cursor,
    event::{EnableBracketedPaste, Event, EventStream},
    execute,
    style::Color,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use editor::Client;
use futures::{future, FutureExt, StreamExt};
use core::panic;
use std::{
    io::{self, Write},
    net::SocketAddrV4,
    str,
};
use text::Text;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

/// Runs a the client side of the editor
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::missing_errors_doc)]
#[tokio::main]
pub async fn run(
    address: SocketAddrV4,
    username: &str,
    password: Option<&str>,
) -> color_eyre::Result<()> {
    let mut out = io::stdout();
    errors::install_hooks()?;

    let mut socket = connect_with_auth(address, username, password)
        .await
        .unwrap();

    let S2C::Full(initial_text) = S2C::<Text>::deserialize(&mut socket).await? else {
        panic!("Initial message in wrong protocol")
    };

    let colors = Vec::<Color>::deserialize(&mut socket).await?;
    let mut app = Client::new_with_buffer(initial_text, colors, Some(socket));

    execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
    enable_raw_mode().unwrap();

    app.redraw(&mut out)?;

    out.execute(cursor::MoveTo(0, 0)).unwrap();

    let mut reader = EventStream::new();
    loop {
        let event = reader.next().fuse();
        if tokio::select! {
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {
                        match event {
                            Event::Key(event) => {
                                if app.handle_keyevent(&event).await? {
                                    break;
                                }
                                true
                            }
                            Event::Mouse(_event) => todo!("No mouse support sorry"),
                            Event::Paste(_data) => todo!("No paste support sorry"),
                            Event::Resize(_width, _height) => true,
                            Event::FocusGained | Event::FocusLost => false,
                        }
                    },
                    Some(Err(e)) => panic!("{e}"),
                    None => panic!("idk what this branch is supposed to handle"),
                }
            },
            x = async {
                if let Some(x) = &mut app.curr_mut().socket{
                    let mut buf= [0];
                    x.reader.peek(&mut buf).await
                } else {
                    future::pending::<()>().await;
                    unreachable!()
                }
            } => {
                x?;
                app.curr_mut().update().await.unwrap()
            },
        } {
            app.curr_mut().recalculate_cursor(terminal::size()?);
            app.redraw(&mut out).unwrap();
            out.flush()?;
        }
    }

    disable_raw_mode().unwrap();
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

async fn connect_with_auth(
    address: SocketAddrV4,
    username: &str,
    password: Option<&str>,
) -> io::Result<TcpStream> {
    let mut stream = TcpStream::connect(address).await?;
    stream.write_all(username.as_bytes()).await?;
    if let Some(password) = password {
        stream.write_u8(254).await?;
        stream.write_all(password.as_bytes()).await?;
    }
    stream.write_all(&[255]).await?;
    stream.flush().await?;
    let ret = stream.read_u8().await?;
    match ret {
        0 => (),
        1 => panic!("You forgot to include a password"),
        2 => panic!("The username, password combination you supplied isn't authorized"),
        3 => {
            panic!("This shouldn't be reachable, but it means that you forgot to supply a password")
        }
        _ => unreachable!(),
    }
    Ok(stream)
}
