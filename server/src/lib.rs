//! A server side for an editor meant to be used by multiple clients
#![feature(try_blocks)]
#[cfg(feature = "security")]
mod security;
use base64::{prelude::BASE64_STANDARD, Engine};
use btep::{c2s::C2S, prelude::S2C, Deserialize};
#[cfg(feature = "security")]
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
#[cfg(feature = "security")]
use std::str::FromStr;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    net::{SocketAddrV4, TcpListener},
    path::Path,
    str,
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::time::sleep;

#[cfg(feature = "security")]
pub use security::add_user;
#[cfg(feature = "security")]
use security::{auth_check, create_tables};

use text::Text;
use tokio_tungstenite::tungstenite::{
    accept_hdr,
    handshake::server::{Request, Response},
};
#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

/// Runs the server for the editor.
#[allow(clippy::missing_panics_doc)]
#[tokio::main]
pub async fn run(
    address: SocketAddrV4,
    path: &Path,
    #[cfg(feature = "security")] pool: SqlitePool,
) {
    #[cfg(feature = "security")]
    let pool = Arc::new(pool);
    #[cfg(feature = "security")]
    create_tables(&pool).await.unwrap();

    let server = TcpListener::bind(address).unwrap();
    let file = File::open(path).unwrap();
    let text = Arc::new(RwLock::new(
        Text::original_from_reader(BufReader::new(file)).unwrap(),
    ));

    let sockets = Arc::new(RwLock::new(Vec::new()));
    let owned_path = path.to_owned();
    let writer_text = Arc::clone(&text);
    tokio::spawn(async move {
        let text = writer_text;
        loop {
            sleep(Duration::from_secs(10)).await;
            let file = OpenOptions::new().write(true).open(&owned_path).unwrap();
            let mut writer = BufWriter::new(file);
            for elem in text.read().unwrap().bufs() {
                writer.write_all(elem.as_bytes()).unwrap();
            }
            info!("Wrote to file")
        }
    });
    for (client_id, stream) in server.incoming().enumerate() {
        debug!("new Client {client_id}");
        let stream = stream.unwrap();
        stream.set_nonblocking(true).unwrap();
        let sockets = Arc::clone(&sockets);
        let text = text.clone();
        #[cfg(feature = "security")]
        let pool = Arc::clone(&pool);
        tokio::spawn(async move {
            let mut username = None;
            let callback = |req: &Request, response: Response| {
                debug!("Received new ws handshake");
                trace!("Received a new ws handshake");
                trace!("The request's path is: {}", req.uri().path());
                trace!("The request's headers are:");
                for (header, _value) in req.headers() {
                    trace!("* {header}");
                }

                #[cfg(feature = "security")]
                {
                    use tokio_tungstenite::tungstenite::http::{self, StatusCode};
                    if let Some(auth) = req.headers().get("Authorization") {
                        if let Some(user) = futures::executor::block_on(auth_check(auth, &pool)) {
                            username = Some(user);
                            Ok(response)
                        } else {
                            Err(http::Response::builder()
                                .status(StatusCode::UNAUTHORIZED)
                                .body(None)
                                .unwrap())
                        }
                    } else {
                        Err(http::Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(None)
                            .unwrap())
                    }
                }

                #[cfg(not(feature = "security"))]
                {
                    username = try {
                        let auth = req.headers().get("Authorization")?;
                        let (credential_type, credentials) =
                            auth.to_str().unwrap().split_once(' ')?;
                        if credential_type != "Basic" {
                            None?;
                        }
                        let base64 = BASE64_STANDARD.decode(credentials).ok()?;
                        let raw = str::from_utf8(base64.as_slice()).ok()?;
                        raw.split_once(':')?.0.to_string()
                    };
                    Ok(response)
                }
            };
            if let Ok(mut websocket) = accept_hdr(stream, callback) {
                {
                    debug!("Connected {:?}", username);
                    let data = text.read().unwrap();
                    // dbg!(&data);
                    websocket.send(S2C::Full(&*data).into_message()).unwrap();
                    // println!("{data:#?}");
                }
                sockets.write().unwrap().push(websocket);
                text.write().unwrap().add_client();
                for (clientnr, client) in sockets.write().as_mut().unwrap().iter_mut().enumerate() {
                    if clientnr == client_id {
                        continue;
                    }
                    client
                        .write(S2C::<&Text>::NewClient.into_message())
                        .unwrap();
                    client.flush().unwrap();
                }
                loop {
                    {
                        let mut socket_lock = sockets.write();
                        let mut_socket = &mut socket_lock.as_mut().unwrap()[client_id];
                        if let Ok(msg) = mut_socket.read() {
                            if msg.is_binary() {
                                let mut binding = text.write().unwrap();
                                let lock = binding.client(client_id);
                                let action = C2S::deserialize(&msg.into_data());
                                match action {
                                    C2S::Char(c) => lock.push_char(c),
                                    C2S::Backspace => lock.backspace(),
                                    C2S::Enter => lock.push_char('\n'),
                                    C2S::EnterInsert(enter_insert) => {
                                        lock.enter_insert(enter_insert);
                                    }
                                }
                                for (clientnr, client) in
                                    socket_lock.as_mut().unwrap().iter_mut().enumerate()
                                {
                                    if clientnr == client_id {
                                        continue;
                                    }
                                    client
                                        .write(
                                            S2C::Update::<&Text>((client_id, action))
                                                .into_message(),
                                        )
                                        .unwrap();
                                    client.flush().unwrap();
                                }
                            } else {
                                warn!("A non-binary message was sent")
                            }
                        }
                    }
                    trace!(
                        "{client_id} {:?}",
                        text.read().unwrap().lines().collect::<Vec<_>>()
                    );
                    // trace!("{client_id} yielded");
                    tokio::task::yield_now().await;
                }
            }
        });
    }
}
