//! A server side for an editor meant to be used by multiple clients
#![feature(never_type)]
#![feature(iter_intersperse)]
#[cfg(feature = "security")]
mod security;

#[cfg(feature = "security")]
use sqlx::SqlitePool;

#[cfg(feature = "security")]
pub use security::add_user;
#[cfg(feature = "security")]
use security::{auth_check, create_tables};

use btep::{c2s::C2S, prelude::S2C, s2c::Inhabitant, Deserialize, Serialize};
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
    sync::{Notify, RwLock},
    time::sleep,
};

use utils::{bufread::BufReaderExt, other::AutoIncrementing};

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

    let files: Arc<RwLock<HashMap<PathBuf, BufferData>>> = Arc::new(RwLock::new(HashMap::new()));

    loop {
        let (stream, _) = server.accept().await.unwrap();
        tokio::spawn(
            handle_connection(
                stream,
                Arc::clone(&files),
                save_interval,
                path.to_path_buf(),
                !is_file,
                #[cfg(feature = "security")]
                Arc::clone(&pool),
            )
            .then(move |output| async move {
                if let Err(e) = output {
                    error!("{e:?}");
                }
            }),
        );
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    files: Arc<RwLock<HashMap<PathBuf, BufferData>>>,
    save_interval: Option<NonZeroU64>,
    path: PathBuf,
    serve_other: bool,
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
                UserAuthError::BadPassword => {
                    warn!("Bad password");
                    stream.write_u8(2).await?;
                }
            }
            stream.flush().await?;
            return Ok(());
        }
    };

    tokio::spawn(handle_client(
        username,
        stream,
        files,
        save_interval,
        path,
        serve_other,
    ));
    Ok(())
}

/// Handles a client connection after it has been verified/authorized
/// # Panics
/// panics if sockets/text is poisoned
async fn handle_client(
    username: String,
    stream: TcpStream,
    files: Arc<RwLock<HashMap<PathBuf, BufferData>>>,
    save_interval: Option<NonZeroU64>,
    path: PathBuf,
    serve_other: bool,
) -> Result<(), io::Error> {
    let (mut read, mut write) = stream.into_split();
    let client_path = if serve_other {
        let C2S::Path(client_path) = C2S::deserialize(&mut read).await? else {
            warn!("Client sent wrong data");
            return Ok(());
        };
        let Ok(canonicalized) = path.join(client_path).canonicalize() else {
            warn!("client path was invalid");
            return Ok(());
        };
        if !(canonicalized.starts_with(path.canonicalize().unwrap())) {
            trace!(
                "client path was invalid: {canonicalized:?} vs {:?}",
                path.canonicalize().unwrap()
            );
            return Ok(());
        }
        canonicalized
    } else {
        C2S::deserialize(&mut read).await?;
        path
    };
    if client_path.is_dir() {
        trace!("serving directory");
        write
            .write_all(
                &S2C::Folder::<&Text>(
                    client_path
                        .read_dir()?
                        .map(|x| x.map(|x| TryInto::<Inhabitant>::try_into(x).unwrap()))
                        .collect::<Result<Vec<_>, io::Error>>()?,
                )
                .serialize(),
            )
            .await?;
        write.flush().await?;
        return Ok(());
    }
    trace!("serving file");
    let C2S::SetColor(new_client_color) = C2S::deserialize(&mut read).await? else {
        warn!("Client set bad color");
        return Ok(());
    };
    {
        let mut lock = files.write().await;
        let entry = lock.entry(client_path.clone()).or_insert_with(|| {
            let file = File::options()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&client_path)
                .unwrap();
            info!("opened new file {client_path:?}");
            let text = Arc::new(RwLock::new(
                Text::original_from_reader(BufReader::new(file)).unwrap(),
            ));
            let notifier = Arc::new(Notify::new());
            let ret = BufferData {
                text: Arc::clone(&text),
                colors: Arc::new(RwLock::new(Vec::new())),
                sockets: Arc::new(RwLock::new(HashMap::new())),
                notifier: Arc::clone(&notifier),
                counter: Arc::new(RwLock::new(AutoIncrementing::default())),
            };

            spawn_saver(text, save_interval, notifier, client_path.clone());
            ret
        });
        let data = {
            let data = entry.text.read().await;
            let full = S2C::Full(&*data);
            full.serialize()
        };

        write.write_all(&data).await?;

        write
            .write_all(&entry.colors.read().await.serialize())
            .await?;
        write.flush().await?;
        debug!("Connected {:?}", username);
        entry.text.write().await.add_client(&username);
        entry.colors.write().await.push(new_client_color);
    }

    for (_, client) in files
        .read()
        .await
        .get(&client_path)
        .unwrap()
        .sockets
        .write()
        .await
        .iter_mut()
    {
        let username = username.clone();
        block_on(async move {
            client
                .write_all(
                    &S2C::<&Text>::NewClient((username.clone(), new_client_color)).serialize(),
                )
                .await?;

            client.flush().await?;
            Ok::<_, io::Error>(())
        })?;
    }

    let self_id = files
        .read()
        .await
        .get(&client_path)
        .unwrap()
        .counter
        .write()
        .await
        .get();
    files
        .read()
        .await
        .get(&client_path)
        .unwrap()
        .sockets
        .write()
        .await
        .insert(self_id, write);
    loop {
        let mut to_remove = Vec::with_capacity(1);
        {
            let action = {
                let action = C2S::deserialize(&mut read).await?;
                let tmp = files.read().await;
                let binding = &mut tmp.get(&client_path).unwrap().text.write().await;
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
                            .get(&client_path)
                            .unwrap()
                            .notifier
                            .notify_one();
                        continue;
                    }
                    C2S::ExitInsert => lock.exit_insert(),
                    C2S::Path(_) | C2S::SetColor(_) => panic!("Can't set pat hnor color here"),
                }
                action
            };

            let tmp = files.read().await;
            let socket_lock = &mut tmp.get(&client_path).unwrap().sockets.write().await;
            for (clientnr, client) in socket_lock.iter_mut() {
                if *clientnr == self_id {
                    continue;
                }
                let result = block_on(
                    client.write_all(&S2C::Update::<&Text>((self_id, action.clone())).serialize()),
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
            let socket_lock = &mut tmp.get(&client_path).unwrap().sockets.write().await;
            for client_to_remove in to_remove {
                info!("removed client {client_to_remove}");
                files
                    .read()
                    .await
                    .get(&client_path)
                    .unwrap()
                    .text
                    .write()
                    .await
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
    T: AsyncRead + AsyncReadExt + AsyncWrite + Unpin + Send,
{
    let mut username = String::new();
    let delim = stream.read_valid_str(&mut username).await?;
    #[cfg(not(feature = "security"))]
    assert_eq!(
        delim,
        Some(255),
        "Client sent wrong byte, maybe you are running with --security"
    );
    #[cfg(feature = "security")]
    {
        assert_eq!(delim, Some(254), "client running without security enabled");
        let mut password = String::new();
        stream.read_valid_str(&mut password).await.unwrap();
        if auth_check(&username, &password, pool).await.is_none() {
            return Err(UserAuthError::BadPassword);
        };
    }
    Ok(username.to_string())
}

enum UserAuthError {
    #[cfg(feature = "security")]
    BadPassword,
    IoError(Error),
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
                tokio::select!(
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
    text: Arc<RwLock<Text>>,
    colors: Arc<RwLock<Vec<Color>>>,
    sockets: Arc<RwLock<HashMap<usize, OwnedWriteHalf>>>,
    notifier: Arc<Notify>,
    counter: Arc<RwLock<AutoIncrementing>>,
}
