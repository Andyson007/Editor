//! A client side for my server side collaboration thing
pub mod editor;
pub mod errors;

use btep::{prelude::S2C, Deserialize};
use crossterm::{
    cursor,
    event::{self, EnableBracketedPaste, Event, EventStream},
    execute,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use editor::Client;
use futures::{FutureExt, StreamExt};
use std::{
    io::{self, Write},
    net::SocketAddrV4,
    str,
    sync::{Arc, RwLock},
    time::Duration,
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

    execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
    enable_raw_mode().unwrap();

    let mut app = Client::new_with_buffer(initial_text, Some(socket));

    app.redraw(&mut out)?;

    out.execute(cursor::MoveTo(0, 0)).unwrap();

    let mut reader = EventStream::new();
    loop {
        let update = app.curr().update().fuse();
        let event = reader.next().fuse();

        if tokio::select! {
            maybe_event = event => {
                match maybe_event {
                    Some(Ok(event)) => {
                        match event {
                            Event::Key(event) => {
                                if app.handle_keyevent(&event).await? {
                                    break;
                                } else {
                                    true
                                }
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
            x = update => x.unwrap(),
        } {
            app.curr().recalculate_cursor(terminal::size()?);
            app.redraw(&mut out).unwrap();
            out.flush()?;
        }
        // if if event::poll(Duration::from_secs(0)).unwrap() {
        //     match event::read()? {
        //         Event::Key(event) => {
        //             if app.handle_keyevent(&event).await? {
        //                 break;
        //             };
        //         }
        //         Event::Mouse(_event) => todo!("No mouse support sorry"),
        //         Event::Paste(_data) => todo!("No paste support sorry"),
        //         Event::Resize(_width, _height) => (),
        //         Event::FocusGained | Event::FocusLost => (),
        //     };
        //     true
        // } else {
        //     app.curr().update().await?
        // } {
        //     app.curr().recalculate_cursor(terminal::size()?);
        //     app.redraw(&mut out).unwrap();
        //     out.flush()?;
        // }
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
    assert_eq!(ret, 0, "You forgot to include a password");
    Ok(stream)
}
