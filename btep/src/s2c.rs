//! Communication from the server to the client
use crossterm::style::Color;
use std::{collections::VecDeque, io, mem};
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
    /// A client has connected with a username and a color
    NewClient((String, Color)),
}

impl<T> Serialize for S2C<T>
where
    T: Serialize,
{
    fn serialize(&self) -> VecDeque<u8> {
        let mut ret = VecDeque::new();
        ret.push_front(6);
        match self {
            Self::Full(x) => {
                ret.push_front(0);
                ret.push_front(7);
                ret.extend(x.serialize());
                ret.push_front(8);
            }
            Self::Update((id, action)) => {
                ret.push_front(1);
                ret.extend((*id as u64).to_be_bytes());
                ret.extend(action.serialize());
            }
            Self::NewClient((username, color)) => {
                ret.push_front(2);
                ret.extend(username.serialize());
                ret.push_back(5);
                ret.extend(color.serialize());
                ret.push_back(5);
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
        assert_eq!(data.read_u8().await?, 6);
        Ok(match data.read_u8().await? {
            0 => {
                assert_eq!(data.read_u8().await?, 7);
                let ret = Self::Full(T::deserialize(data).await?);
                assert_eq!(data.read_u8().await?, 8);
                ret
            }
            1 => {
                let mut buf = [0; mem::size_of::<u64>()];
                data.read_exact(&mut buf).await?;
                let id = u64::from_be_bytes(buf) as usize;
                let action = C2S::deserialize(data).await?;
                Self::Update((id, action))
            }
            2 => {
                let username = String::deserialize(data).await?;
                assert_eq!(data.read_u8().await?, 5);
                let color = <Color as Deserialize>::deserialize(data).await?;
                assert_eq!(data.read_u8().await?, 5);
                Self::NewClient((username, color))
            }
            x => panic!("An invalid specifier was found ({x})"),
        })
    }
}
