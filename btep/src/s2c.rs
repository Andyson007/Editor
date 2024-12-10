//! Communication from the server to the client
use crossterm::style::Color;
use std::{
    collections::VecDeque,
    io,
    mem::{self, MaybeUninit},
};
use tokio::io::AsyncReadExt;
use {crate::c2s::C2S, crate::Deserialize, crate::Serialize};

/// S2C or Server to Client
/// Encodes information that originates from the server and sendt to the client
pub enum S2C<T> {
    /// The initial message over the websocket.
    /// This should usually be the entireity of the file
    Full(T),
    /// A client has made an update to their buffer
    Update((usize, C2S)),
    /// A client has connected
    NewClient(Color),
}

impl<T> Serialize for S2C<T>
where
    T: Serialize,
{
    fn serialize(&self) -> VecDeque<u8> {
        let mut ret = VecDeque::new();
        match self {
            Self::Full(x) => {
                ret.push_front(0);
                ret.extend(x.serialize());
            }
            Self::Update((id, action)) => {
                ret.push_front(1);
                ret.extend((*id as u64).to_be_bytes());
                ret.extend(action.serialize());
            }
            Self::NewClient(color) => {
                ret.push_front(2);
                ret.extend(unsafe {
                    mem::transmute::<Color, [u8; mem::size_of::<Color>()]>(*color)
                });
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
                let mut buf = [0; mem::size_of::<Color>()];
                data.read_exact(&mut buf).await?;
                Self::NewClient(unsafe {
                    mem::transmute::<[u8; mem::size_of::<Color>()], Color>(buf)
                })
            }
            x => panic!("An invalid specifier was found ({x})"),
        })
    }
}
