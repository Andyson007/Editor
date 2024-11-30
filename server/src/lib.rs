//! A server side for an editor meant to be used by multiple clients
#![feature(try_blocks)]
#[cfg(feature = "security")]
mod security;
use base64::{prelude::BASE64_STANDARD, Engine};
use btep::Btep;
#[cfg(feature = "security")]
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
#[cfg(feature = "security")]
use std::str::FromStr;
use std::{
    fs::File,
    io::BufReader,
    net::TcpListener,
    str,
    sync::{Arc, RwLock},
};

#[cfg(feature = "security")]
pub use security::add_user;
#[cfg(feature = "security")]
use security::{auth_check, create_tables};

use text::Text;
// I want to keep the tracing tools in scope
use tokio_tungstenite::tungstenite::{
    accept_hdr,
    handshake::server::{Callback, Request, Response},
};
#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

/// Runs the server for the editor.
#[allow(clippy::missing_panics_doc)]
#[tokio::main]
pub async fn run(#[cfg(feature = "security")] pool: SqlitePool) {
    #[cfg(feature = "security")]
    let pool = Arc::new(pool);
    #[cfg(feature = "security")]
    create_tables(&pool).await.unwrap();

    let server = TcpListener::bind("127.0.0.1:3012").unwrap();
    let file = File::open("./file.txt").unwrap();
    let text = Arc::new(RwLock::new(
        Text::original_from_reader(BufReader::new(file)).unwrap(),
    ));
    for stream in server.incoming() {
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
                    websocket.send(Btep::Full(&*data).into_message()).unwrap();
                }
                loop {
                    let msg = websocket.read().unwrap();
                    if msg.is_binary() || msg.is_text() {
                        debug!("{msg:?}");
                    }
                }
            }
        });
    }
}
