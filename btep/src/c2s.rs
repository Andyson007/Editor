//! mdoule for client updates sendt to the server

use std::{io, path::PathBuf, str::FromStr};

use tokio::io::AsyncReadExt;
use utils::other::CursorPos;

use crate::{Deserialize, Serialize};

/// S2C or Server to Client
/// Encodes information that originates from the client and sendt to the server
#[derive(Clone, Debug)]
pub enum C2S {
    /// The client wrote a character
    Char(char),
    /// The client pressed backspace
    Backspace(usize),
    /// The client pressed enter
    Enter,
    /// The client pressed entered insert mode at a position
    // TODO: this should use the `EnterInsert` instead which should be more immune to server-client
    // desync
    EnterInsert(CursorPos),
    /// A client left insert mode
    ExitInsert,
    /// Force a save to happen
    Save,
    /// A path to the file that you want to request
    Path(PathBuf),
}

// #[derive(Clone, Copy, Debug)]
// /// A representation of entering insert mode which shuold be more accurate than just sending the
// /// clients cursors position
// pub struct EnterInsert {
//     /// The id of the buffer that was split
//     pub id: usize,
//     /// The offset in that buffer
//     /// Option because appending a character has special behaviour
//     pub offset: Option<usize>,
// }
//
// impl Serialize for EnterInsert {
//     fn serialize(&self) -> VecDeque<u8> {
//         todo!()
//     }
// }
//
// impl Deserialize for EnterInsert {
//     async fn deserialize<T>(_data: &mut T) -> io::Result<Self>
//     where
//         T: AsyncReadExt,
//         Self: Sized,
//     {
//         todo!()
//     }
// }

impl Serialize for C2S {
    fn serialize(&self) -> Vec<u8> {
        match self {
            Self::Char(c) => std::iter::once(1).chain(c.serialize()).collect(),
            Self::EnterInsert(a) => std::iter::once(2).chain(a.serialize()).collect(),
            Self::Save => [3].into(),
            Self::ExitInsert => [4].into(),
            Self::Path(path) => std::iter::once(5)
                .chain(path.to_str().unwrap().serialize())
                .collect(),
            Self::Backspace(swaps) => std::iter::once(8)
                .chain((*swaps as u64).to_be_bytes())
                .collect(),
            Self::Enter => [10].into(),
        }
    }
}

impl Deserialize for C2S {
    async fn deserialize<T>(data: &mut T) -> io::Result<Self>
    where
        T: AsyncReadExt + Unpin + Send,
        Self: Sized,
    {
        Ok(match data.read_u8().await? {
            1 => Self::Char(
                char::from_u32(data.read_u32().await?).expect("An invalid char was supplied"),
            ),
            2 => Self::EnterInsert(CursorPos::deserialize(data).await?),
            3 => Self::Save,
            4 => Self::ExitInsert,
            5 => Self::Path(PathBuf::from_str(&String::deserialize(data).await?).unwrap()),
            8 => Self::Backspace(data.read_u64().await? as usize),
            10 => Self::Enter,
            x => unreachable!("{x}"),
        })
    }
}
