//! Communication from the server to the client
use crossterm::style::Color;
use std::{
    ffi::OsString,
    fs::{self, DirEntry},
    io, mem,
};
use tokio::io::AsyncReadExt;
use {crate::c2s::C2S, crate::Deserialize, crate::Serialize};

/// S2C or Server to Client
/// Encodes information that originates from the server and sendt to the client
pub enum S2C<T> {
    /// Sends a full description of the layout of a file.
    Full(T),
    Folder(Vec<Inhabitant>),
    /// A client has made an update to their buffer
    Update((usize, C2S)),
    /// A client has connected with a username and a color
    NewClient((String, Color)),
}

pub struct Inhabitant {
    name: OsString,
    is_folder: bool,
    // TODO: Prob add like filesize stuff too
}

impl Serialize for Inhabitant {
    fn serialize(&self) -> Vec<u8> {
        let mut ret = Vec::new();
        ret.extend(self.name.to_str().unwrap().serialize());
        ret.extend(self.is_folder.serialize());
        ret
    }
}

impl Deserialize for Inhabitant {
    async fn deserialize<T>(data: &mut T) -> io::Result<Self>
    where
        Self: Sized,
        T: AsyncReadExt + Unpin + Send,
    {
        let name = String::deserialize(data).await?;
        let is_folder = bool::deserialize(data).await?;
        Ok(Self {
            name: name.into(),
            is_folder,
        })
    }
}

impl TryFrom<DirEntry> for Inhabitant {
    type Error = io::Error;
    fn try_from(val: DirEntry) -> Result<Self, Self::Error> {
        Ok(Self {
            name: val.file_name(),
            is_folder: fs::metadata(val.path())?.is_dir(),
        })
    }
}

impl<T> Serialize for S2C<T>
where
    T: Serialize,
{
    fn serialize(&self) -> Vec<u8> {
        let mut ret = Vec::new();
        match self {
            Self::Full(x) => {
                ret.push(0);
                ret.extend(x.serialize());
            }
            Self::Update((id, action)) => {
                ret.push(1);
                ret.extend((*id as u64).to_be_bytes());
                ret.extend(action.serialize());
            }
            Self::NewClient((username, color)) => {
                ret.push(2);
                ret.extend(username.serialize());
                ret.extend(color.serialize());
            }
            Self::Folder(x) => {
                ret.push(3);
                ret.extend(x.serialize());
            }
        };
        ret
    }
}

impl<T> Deserialize for S2C<T>
where
    T: Deserialize,
{
    async fn deserialize<D>(data: &mut D) -> io::Result<Self>
    where
        D: AsyncReadExt + Unpin + Send,
        Self: Sized,
    {
        Ok(match data.read_u8().await? {
            0 => Self::Full(T::deserialize(data).await?),
            1 => {
                let mut buf = [0; mem::size_of::<u64>()];
                data.read_exact(&mut buf).await?;
                let id = u64::from_be_bytes(buf) as usize;
                let action = C2S::deserialize(data).await?;
                Self::Update((id, action))
            }
            2 => {
                let username = String::deserialize(data).await?;
                let color = <Color as Deserialize>::deserialize(data).await?;
                Self::NewClient((username, color))
            }
            3 => Self::Folder(Vec::deserialize(data).await?),
            x => panic!("An invalid specifier was found ({x})"),
        })
    }
}
