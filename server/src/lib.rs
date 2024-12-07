//! A server side for an editor meant to be used by multiple clients
#![feature(try_blocks)]
#[cfg(feature = "security")]
mod security;
use btep::{c2s::C2S, prelude::S2C, Deserialize, Serialize};
use futures::executor::block_on;
#[cfg(feature = "security")]
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
#[cfg(feature = "security")]
use std::str::FromStr;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Error, Write},
    net::SocketAddrV4,
    num::NonZeroU64,
    path::Path,
    sync::{Arc, RwLock},
    time::Duration,
};
use text::Text;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{tcp::OwnedWriteHalf, TcpListener},
    select,
    sync::Notify,
    time::sleep,
};

#[cfg(feature = "security")]
pub use security::add_user;
#[cfg(feature = "security")]
use security::{auth_check, create_tables};

#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};

/// Runs the server for the editor.
#[allow(clippy::missing_panics_doc)]
#[tokio::main]
pub async fn run(
    save_interval: Option<NonZeroU64>,
    address: SocketAddrV4,
    path: &Path,
    #[cfg(feature = "security")] pool: SqlitePool,
) {
    #[cfg(feature = "security")]
    let pool = Arc::new(pool);
    #[cfg(feature = "security")]
    create_tables(&pool).await.unwrap();

    let server = TcpListener::bind(address).await.unwrap();
    let file = File::open(path).unwrap();
    let text = Arc::new(RwLock::new(
        Text::original_from_reader(BufReader::new(file)).unwrap(),
    ));

    let sockets = Arc::new(RwLock::new(HashMap::<usize, OwnedWriteHalf>::new()));
    let owned_path = path.to_owned();
    let writer_text = Arc::clone(&text);

    let save_notify = Arc::new(Notify::new());
    let save_notify_reader = Arc::clone(&save_notify);
    tokio::spawn(async move {
        let text = writer_text;
        loop {
            if let Some(x) = save_interval {
                select!(
                    () = sleep(Duration::from_secs(x.get())) => (),
                    () = save_notify_reader.notified() => {}
                );
            } else {
                save_notify_reader.notified().await;
            }
            let file = OpenOptions::new().write(true).open(&owned_path).unwrap();
            let mut writer = BufWriter::new(file);
            let buf_iter = text.read().unwrap().bufs();
            for elem in buf_iter {
                writer.write_all(elem.as_bytes()).unwrap();
            }
            info!("Wrote to file");
        }
    });
    for client_id in 0.. {
        let save_notify = Arc::clone(&save_notify);
        // for (client_id, stream) in server.enumerate() {
        let (mut stream, _) = server.accept().await.unwrap();
        debug!("new Client {client_id}");

        let sockets = Arc::clone(&sockets);
        let text = text.clone();

        #[cfg(feature = "security")]
        let pool = Arc::clone(&pool);

        let username = match authorize(
            &mut stream,
            #[cfg(feature = "security")]
            &pool,
        )
        .await
        {
            Ok(x) => {
                stream.write_u8(0).await.unwrap();
                stream.flush().await.unwrap();
                x
            }
            Err(x) => {
                match x {
                    UserAuthError::IoError(e) => panic!("{e:?}"),
                    #[cfg(feature = "security")]
                    UserAuthError::BadPassword => {
                        warn!("client {client_id} forgot to include a password");
                        stream.write_u8(2).await.unwrap();
                    }
                    #[cfg(feature = "security")]
                    UserAuthError::NeedsPassword => {
                        warn!("client {client_id} forgot to include a password");
                        stream.write_u8(1).await.unwrap();
                    }
                }
                stream.flush().await.unwrap();
                continue;
            }
        };

        let (mut read, mut write) = stream.into_split();
        tokio::spawn(async move {
            {
                debug!("{client_id} Connected {:?}", username);
                let mut data = {
                    let data = text.read().unwrap();
                    let full = S2C::Full(&*data);
                    full.serialize()
                };
                // dbg!(&data);
                write.write_all(data.make_contiguous()).await.unwrap();
                write.flush().await.unwrap();
                // println!("{data:#?}");
            }
            debug_assert_eq!(text.write().unwrap().add_client(), client_id);
            for (_, client) in sockets.write().as_mut().unwrap().iter_mut() {
                block_on(async {
                    client
                        .write_all(S2C::<&Text>::NewClient.serialize().make_contiguous())
                        .await
                        .unwrap();
                    client.flush().await.unwrap();
                });
            }
            sockets.write().unwrap().insert(client_id, write);
            loop {
                let mut to_remove = Vec::with_capacity(1);
                {
                    let action = {
                        let action = C2S::deserialize(&mut read).await.unwrap();
                        let mut binding = text.write().unwrap();
                        let lock = binding.client(client_id);
                        match action {
                            C2S::Char(c) => lock.push_char(c),
                            C2S::Backspace => drop(lock.backspace()),
                            C2S::Enter => lock.push_char('\n'),
                            C2S::EnterInsert(enter_insert) => {
                                lock.enter_insert(enter_insert);
                            }
                            C2S::Save => {
                                save_notify.notify_one();
                                continue;
                            }
                        }
                        action
                    };

                    let mut socket_lock = sockets.write().unwrap();
                    for (clientnr, client) in socket_lock.iter_mut() {
                        if *clientnr == client_id {
                            continue;
                        }
                        let result = block_on(
                            client.write_all(
                                S2C::Update::<&Text>((client_id, action))
                                    .serialize()
                                    .make_contiguous(),
                            ),
                        );
                        match result {
                            Ok(()) => block_on(async { client.flush().await.unwrap() }),
                            Err(e) => {
                                to_remove.push(*clientnr);
                                warn!("{client_id}: {e}");
                            }
                        };
                    }
                }
                {
                    let mut lock = sockets.write().unwrap();
                    for x in to_remove {
                        info!("removed client {x}");
                        lock.remove(&x);
                    }
                }
                trace!(
                    "{client_id} {:?}",
                    text.read().unwrap().lines().collect::<Vec<_>>()
                );
            }
        });
    }
}

async fn authorize<T>(
    stream: &mut T,
    #[cfg(feature = "security")] pool: &SqlitePool,
) -> Result<String, UserAuthError>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    let mut buf = Vec::new();
    stream.read_buf(&mut buf).await?;
    let mut iter = buf.utf8_chunks();
    let username = iter.next().unwrap().valid();
    #[cfg(feature = "security")]
    {
        let Some(password) = iter.next() else {
            return Err(UserAuthError::NeedsPassword);
        };
        if auth_check(username, password.valid(), pool).await.is_none() {
            return Err(UserAuthError::BadPassword);
        };
    }
    Ok(username.to_string())
}

enum UserAuthError {
    #[cfg(feature = "security")]
    BadPassword,
    #[cfg(feature = "security")]
    NeedsPassword,
    IoError(Error),
}

impl From<std::io::Error> for UserAuthError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}
