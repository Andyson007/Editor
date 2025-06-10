//! A client side for my server side collaboration thing
pub mod editor;
pub mod errors;

use crossterm::{
    event::{Event, EventStream},
    execute,
    style::Color,
    terminal::{
        self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use editor::App;
use futures::{future, FutureExt, StreamExt};
use std::{
    io::{self, Write},
    net::SocketAddrV4,
    path::Path,
    str,
};

use tokio::{io::Interest, time};

/// Runs a the client side of the editor
#[allow(clippy::missing_panics_doc)]
#[allow(clippy::missing_errors_doc)]
#[tokio::main]
pub async fn run(
    address: SocketAddrV4,
    username: &str,
    #[cfg(feature = "security")] password: &str,
    color: &Color,
    path: &Path,
) -> color_eyre::Result<()> {
    let mut out = io::stdout();
    errors::install_hooks()?;

    let mut app = App::new(
        username.to_string(),
        #[cfg(feature = "security")]
        password,
        address,
        color,
        path,
    )
    .await?;

    execute!(out, EnterAlternateScreen)?;
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
            r = async {
                if let Some(x) = &mut app.client.buffers[app.client.current_buffer].socket{
                    x.reader.ready(Interest::READABLE).await
                } else {
                    future::pending::<()>().await;
                    unreachable!()
                }
            } => {
                if r?.is_read_closed() {
                    break;
                } else {
                    app.client.curr_mut().update().await?;
                    Ok::<bool, io::Error>(true)
                }
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
