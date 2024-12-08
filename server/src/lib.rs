//! A server side for an editor meant to be used by multiple clients
#![feature(never_type)]
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
    io::{self, BufReader, BufWriter, Error, Write},
    net::SocketAddrV4,
    num::NonZeroU64,
    path::Path,
    str::Utf8Chunk,
    sync::{Arc, RwLock},
    time::Duration,
};
use text::Text;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{tcp::OwnedWriteHalf, TcpListener, TcpStream},
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
    create_tables(&pool)
        .await
        .expect("Failed to create the users table");

    let server = TcpListener::bind(address).await.unwrap();
    let file = File::options()
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap();
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
                    UserAuthError::IoError(e) => warn!("{client_id} had an IoError: `{e:?}`"),
                    #[cfg(feature = "security")]
                    UserAuthError::NeedsPassword => {
                        warn!("client {client_id} forgot to include a password");
                        stream.write_u8(1).await.unwrap();
                    }
                    #[cfg(feature = "security")]
                    UserAuthError::BadPassword => {
                        warn!("client {client_id} forgot to include a password");
                        stream.write_u8(2).await.unwrap();
                    }
                    UserAuthError::MissingUsername => {
                        warn!("{client_id} Forgot to supply a username");
                        stream.write_u8(3).await.unwrap();
                    }
                }
                stream.flush().await.unwrap();
                continue;
            }
        };

        tokio::spawn(handle_client(
            client_id,
            username,
            text,
            stream,
            sockets,
            save_notify,
        ));
    }
}

/// Handles a client connection after it has been verified/authorized
/// # Panics
/// panics if sockets/text is poisoned
async fn handle_client(
    client_id: usize,
    username: String,
    text: Arc<RwLock<Text>>,
    stream: TcpStream,
    sockets: Arc<RwLock<HashMap<usize, OwnedWriteHalf>>>,
    save_notify: Arc<Notify>,
) -> Result<!, io::Error> {
    let (mut read, mut write) = stream.into_split();

    {
        debug!("{client_id} Connected {:?}", username);
        let mut data = {
            let data = text.read().unwrap();
            let full = S2C::Full(&*data);
            full.serialize()
        };
        // dbg!(&data);
        write.write_all(data.make_contiguous()).await?;
        write.flush().await?;
        // println!("{data:#?}");
    }
    debug_assert_eq!(text.write().unwrap().add_client(), client_id);
    for (_, client) in sockets.write().as_mut().unwrap().iter_mut() {
        block_on(async {
            client
                .write_all(S2C::<&Text>::NewClient.serialize().make_contiguous())
                .await?;
            client.flush().await?;
            Ok::<_, io::Error>(())
        })?;
    }
    sockets.write().unwrap().insert(client_id, write);
    loop {
        let mut to_remove = Vec::with_capacity(1);
        {
            let action = {
                let action = C2S::deserialize(&mut read).await?;
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
                    Ok(()) => block_on(async { client.flush().await })?,
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
    }
}

/// Checks whether a socket supplies proper authorization credentials
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
    let Some(username) = iter.next().map(|x| x.valid()) else {
        return Err(UserAuthError::MissingUsername);
    };
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
    MissingUsername,
}

impl From<std::io::Error> for UserAuthError {
    fn from(e: std::io::Error) -> Self {
        Self::IoError(e)
    }
}
