use core::str;
use std::io::{self, ErrorKind};

use tokio::io::{AsyncRead, AsyncReadExt};

pub trait BufReaderExt {
    /// Returns none if the buffer was read to completion
    fn read_valid_str(
        &mut self,
        buffer: &mut String,
    ) -> impl std::future::Future<Output = io::Result<Option<u8>>> + Send;
}

impl<T> BufReaderExt for T
where
    T: AsyncRead + Unpin + Send,
{
    async fn read_valid_str(&mut self, buffer: &mut String) -> io::Result<Option<u8>> {
        loop {
            let first = match self.read_u8().await {
                Ok(x) => x,
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
                Err(e) => return Err(e),
            };
            let leading = first.leading_ones() as usize;
            if leading > 4 {
                return Ok(Some(first));
            }
            let mut buf = [0; 4];
            buf[0] = first;
            for x in buf.iter_mut().take(leading).skip(1) {
                *x = self.read_u8().await?;
            }
            match str::from_utf8(&buf[..=leading]) {
                Ok(x) => buffer.push_str(x),
                Err(_) => return Ok(Some(first)),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use futures::executor::block_on;
    use tokio::io::BufReader;

    use crate::bufread::BufReaderExt;

    #[test]
    fn all_valid() {
        let mut reader = BufReader::new(&b"andy"[..]);
        let mut buf = String::new();
        let blocking = block_on(reader.read_valid_str(&mut buf)).unwrap();
        assert_eq!(&buf, "andy");
        assert_eq!(blocking, None);
    }

    #[test]
    fn invalid() {
        let mut reader = BufReader::new(&b"andy\xFF"[..]);
        let mut buf = String::new();
        let blocking = block_on(reader.read_valid_str(&mut buf)).unwrap();
        assert_eq!(&buf, "andy");
        assert_eq!(blocking, Some(0xff));
    }
}
