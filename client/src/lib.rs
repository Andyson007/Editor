//! A client side for my server side collaboration thing
pub mod editor;
pub mod errors;

use btep::{prelude::S2C, Deserialize};
use core::panic;
use crossterm::{
    event::{EnableBracketedPaste, Event, EventStream},
    execute,
    style::Color,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use editor::App;
use futures::{future, FutureExt, StreamExt};
use std::{
    io::{self, Write}, net::SocketAddrV4, path::{Path, PathBuf}, str
};
use text::Text;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time,
};

/// Runs a the client side of the editor
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::missing_errors_doc)]
#[tokio::main]
pub async fn run(
    address: SocketAddrV4,
    username: &str,
    password: Option<&str>,
    path: &Path,
) -> color_eyre::Result<()> {
    let mut out = io::stdout();
    errors::install_hooks()?;

    let Ok(socket) = connect_with_auth(address, username, password).await else {
        panic!("Failed to connect to the server. Maybe the server is not running?")
    };

    let mut app = App::new(username.to_string(), socket, path).await?;

    execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
    enable_raw_mode().unwrap();

    app.client.redraw(&mut out)?;

    let mut reader = EventStream::new();
    loop {
        let event = reader.next().fuse();
        if tokio::select! {
            maybe_event = event => {
                Ok(match maybe_event {
                    Some(Ok(event)) => {
                        match event {
                            Event::Key(event) => {
                                app.handle_keyevent(&event).await?
                            }
                            Event::Mouse(_event) => todo!("No mouse support sorry"),
                            Event::Paste(_data) => todo!("No paste support sorry"),
                            Event::Resize(_width, _height) => true,
                            Event::FocusGained | Event::FocusLost => false,
                        }
                    },
                    Some(Err(e)) => panic!("{e}"),
                    None => panic!("idk what this branch is supposed to handle"),
                })
            },
            length = async {
                if let Some(x) = &mut app.client.buffers[app.client.current_buffer].socket{
                    let mut buf = [0];
                    x.reader.peek(&mut buf).await
                } else {
                    future::pending::<()>().await;
                    unreachable!()
                }
            } => {
                assert_eq!(length?, 1, "The server disconnected");
                app.client.curr_mut().update().await?;
                Ok::<bool, io::Error>(true)
            },
            _ = async {
                if let Some(timer) = app.client.modeinfo.timer.as_ref() {
                    time::sleep_until(timer.deadline()).await;
                } else {
                    future::pending::<()>().await;
                    unreachable!()
                }
            } => {
                app.execute_keyevents().await?;

                Ok(true)
            }
        }? {
            if app.client.buffers.is_empty() {
                break;
            }
            let size = terminal::size()?;
            app.client
                .curr_mut()
                .recalculate_cursor((size.0, size.1 - 1))?;
            app.client.redraw(&mut out)?;
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
