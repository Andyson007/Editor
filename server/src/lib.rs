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
    fs::File,
    io::BufReader,
    net::{SocketAddrV4, TcpListener},
    path::Path,
    str,
    sync::{Arc, RwLock},
};

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

    let mut sockets = Arc::new(RwLock::new(Vec::new()));

    for (client_id, stream) in server.incoming().enumerate() {
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
            if let Ok(mut websocket) = accept_hdr(stream.unwrap(), callback) {
                {
                    debug!("Connected {:?}", username);
                    let data = text.read().unwrap();
                    // dbg!(&data);
                    websocket.send(S2C::Full(&*data).into_message()).unwrap();
                    println!("{data:#?}");
                }
                sockets.write().unwrap().push(websocket);
                text.write().unwrap().add_client();
                loop {
                    let msg = sockets.write().unwrap()[client_id].read().unwrap();
                    if msg.is_binary() {
                        let mut binding = text.write().unwrap();
                        let lock = binding.client(client_id);
                        let action = C2S::deserialize(&msg.into_data());
                        match action {
                            C2S::Char(c) => lock.push_char(c),
                            C2S::Backspace => lock.backspace(),
                            C2S::Enter => lock.push_char('\n'),
                            C2S::EnterInsert(enter_insert) => drop(lock.enter_insert(enter_insert)),
                        }
                        for client in sockets.write().unwrap().iter_mut().take(client_id).skip(1) {
                            client
                                .write(S2C::Update::<&Text>((client_id, action)).into_message())
                                .unwrap();
                        }
                    } else {
                        warn!("A non-binary message was sent")
                    }
                    trace!("{:?}", text.read().unwrap().lines().collect::<Vec<_>>());
                }
            }
        });
    }
}
