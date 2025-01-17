//! A server side for an editor meant to be used by multiple clients
#![feature(never_type)]
#[cfg(feature = "security")]
mod security;

#[cfg(feature = "security")]
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
#[cfg(feature = "security")]
use std::str::FromStr;

#[cfg(feature = "security")]
pub use security::add_user;
#[cfg(feature = "security")]
use security::{auth_check, create_tables};

use btep::{c2s::C2S, prelude::S2C, Deserialize, Serialize};
use crossterm::style::Color;
use futures::{executor::block_on, FutureExt};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, BufReader, BufWriter, Error, Write},
    net::SocketAddrV4,
    num::NonZeroU64,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use text::Text;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{tcp::OwnedWriteHalf, TcpListener, TcpStream},
    select,
    sync::{Notify, RwLock},
    time::sleep,
};

use utils::other::AutoIncrementing;

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
    let is_file = fs::metadata(path).unwrap().file_type().is_file();
    if !is_file {
        assert!(
            fs::metadata(path).unwrap().file_type().is_dir(),
            "I don't handle non-file nor non-dir stuff"
        );
    }

    let files: Arc<RwLock<HashMap<PathBuf, Arc<RwLock<BufferData>>>>> =
        Arc::new(RwLock::new(HashMap::new()));

    for client_id in 0.. {
        let (stream, _) = server.accept().await.unwrap();
        tokio::spawn(
            handle_connection(
                stream,
                Arc::clone(&files),
                #[cfg(feature = "security")]
                Arc::clone(&pool),
            )
            .then(move |output| async move {
                if let Err(e) = output {
                    error!("{client_id} {e:?}");
                }
            }),
        );
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    files: Arc<RwLock<HashMap<PathBuf, Arc<RwLock<BufferData>>>>>,
    #[cfg(feature = "security")] pool: Arc<SqlitePool>,
) -> io::Result<()> {
    debug!("new Client");

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
            stream.write_u8(0).await?;
            stream.flush().await?;
            x
        }
        Err(x) => {
            match x {
                UserAuthError::IoError(e) => warn!("IoError: `{e:?}`"),
                #[cfg(feature = "security")]
                UserAuthError::NeedsPassword => {
                    warn!("Forgotten password");
                    stream.write_u8(1).await?;
                }
                #[cfg(feature = "security")]
                UserAuthError::BadPassword => {
                    warn!("Bad password");
                    stream.write_u8(2).await?;
                }
                UserAuthError::MissingUsername => {
                    warn!("Missing username");
                    stream.write_u8(3).await?;
                }
            }
            stream.flush().await?;
            return Ok(());
        }
    };

    tokio::spawn(handle_client(username, stream, files));
    Ok(())
}

/// Handles a client connection after it has been verified/authorized
/// # Panics
/// panics if sockets/text is poisoned
async fn handle_client(
    username: String,
    stream: TcpStream,
    files: Arc<RwLock<HashMap<PathBuf, Arc<RwLock<BufferData>>>>>,
) -> Result<!, io::Error> {
    let (mut read, mut write) = stream.into_split();

    let C2S::Path(path) = C2S::deserialize(&mut read).await? else {
        panic!();
    };
    println!("{path:?}");
    {
        let mut lock = files.write().await;
        let entry = lock.entry(path.clone()).or_insert_with(|| {
            let file = File::options()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            Arc::new(RwLock::new(BufferData {
                text: Text::original_from_reader(BufReader::new(file)).unwrap(),
                colors: Vec::new(),
                sockets: HashMap::new(),
                notifier: Notify::new(),
                counter: AutoIncrementing::default(),
            }))
        });
        let mut entry_lock = entry.write().await;
        let data = {
            let data = &entry_lock.text;
            let full = S2C::Full(data);
            full.serialize()
        };
        // dbg!(&data);

        write.write_all(&data).await?;

        write.write_all(&entry_lock.colors.serialize()).await?;
        write.flush().await?;
        // println!("{data:#?}");
        debug!("Connected {:?}", username);
        entry_lock.text.add_client(&username);
        entry_lock.colors.push(Color::Green);
    }

    for (_, client) in files
        .read()
        .await
        .get(&path)
        .unwrap()
        .write()
        .await
        .sockets
        .iter_mut()
    {
        let username = username.clone();
        block_on(async move {
            client
                .write_all(&S2C::<&Text>::NewClient((username.clone(), Color::Green)).serialize())
                .await?;

            client.flush().await?;
            Ok::<_, io::Error>(())
        })?;
    }

    let self_id = files
        .read()
        .await
        .get(&path)
        .unwrap()
        .write()
        .await
        .counter
        .get();
    files
        .read()
        .await
        .get(&path)
        .unwrap()
        .write()
        .await
        .sockets
        .insert(self_id, write);
    loop {
        let mut to_remove = Vec::with_capacity(1);
        {
            let action = {
                let action = C2S::deserialize(&mut read).await?;
                let tmp = files.read().await;
                let binding = &mut tmp.get(&path).unwrap().write().await.text;
                let lock = binding.client_mut(self_id);
                match action {
                    C2S::Char(c) => lock.push_char(c),
                    C2S::Backspace(swaps) => drop(lock.backspace_with_swaps(swaps)),
                    C2S::Enter => lock.push_char('\n'),
                    C2S::EnterInsert(enter_insert) => {
                        lock.enter_insert(enter_insert);
                    }
                    C2S::Save => {
                        files
                            .read()
                            .await
                            .get(&path)
                            .unwrap()
                            .write()
                            .await
                            .notifier
                            .notify_one();
                        continue;
                    }
                    C2S::ExitInsert => lock.exit_insert(),
                    C2S::Path(_) => todo!(),
                }
                action
            };

            let tmp = files.read().await;
            let socket_lock = &mut tmp.get(&path).unwrap().write().await.sockets;
            for (clientnr, client) in socket_lock.iter_mut() {
                if *clientnr == self_id {
                    continue;
                }
                let result = block_on(
                    client
                        .write_all(&S2C::Update::<&Text>((self_id, action.clone())).serialize()),
                );
                match result {
                    Ok(()) => block_on(client.flush())?,
                    Err(e) => {
                        to_remove.push(*clientnr);
                        warn!("{self_id}: {e}");
                    }
                };
            }
        }
        {
            let tmp = files.read().await;
            let socket_lock = &mut tmp.get(&path).unwrap().write().await.sockets;
            for client_to_remove in to_remove {
                info!("removed client {client_to_remove}");
                files
                    .read()
                    .await
                    .get(&path)
                    .unwrap()
                    .write()
                    .await
                    .text
                    .client_mut(client_to_remove)
                    .exit_insert();

                for (clientnr, client) in socket_lock.iter_mut() {
                    if *clientnr == client_to_remove {
                        continue;
                    }
                    let result = block_on(client.write_all(
                        &S2C::Update::<&Text>((client_to_remove, C2S::ExitInsert)).serialize(),
                    ));
                    match result {
                        Ok(()) => block_on(client.flush())?,
                        Err(_) => {
                            // The responsible thing to do would be to remove it here, but it'll be
                            // removed anyways at the next iteration
                        }
                    };
                }
                socket_lock.remove(&client_to_remove);
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

fn spawn_saver(
    text: Arc<RwLock<Text>>,
    save_interval: Option<NonZeroU64>,
    save_notify: Arc<Notify>,
    path: PathBuf,
) {
    tokio::spawn(async move {
        loop {
            if let Some(x) = save_interval {
                select!(
                    () = sleep(Duration::from_secs(x.get())) => (),
                    () = save_notify.notified() => {}
                );
            } else {
                save_notify.notified().await;
            }
            let file = OpenOptions::new().write(true).open(&path).unwrap();
            let mut writer = BufWriter::new(file);
            let buf_iter = text.read().await.bufs().map(|x| x.read().text.clone());
            for elem in buf_iter {
                writer.write_all(elem.as_bytes()).unwrap();
            }
            info!("Wrote to file");
        }
    });
}

struct BufferData {
    text: Text,
    colors: Vec<Color>,
    sockets: HashMap<usize, OwnedWriteHalf>,
    notifier: Notify,
    counter: AutoIncrementing,
}
