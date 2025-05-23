//! Provides extensions for reading method such as `Read` and `AsyncRead`
use core::str;
use std::io::{self, ErrorKind};

use tokio::io::{AsyncRead, AsyncReadExt};

/// Extensions for `AsyncRead`
pub trait BufReaderExt {
    /// Reads from a buffer until something that isn't utf-8 compliant is found.
    /// Errors are ill-defined for overlong-encoded stuff
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
            if leading == 1 {
                return Ok(Some(first));
            }

            if leading > 4 {
                return Ok(Some(first));
            }

            let byte_width = if leading == 0 { 1 } else { leading };

            let mut buf = [0; 4];
            buf[0] = first;
            for x in buf.iter_mut().take(byte_width).skip(1) {
                *x = self.read_u8().await?;
            }
            let utf_slice = &buf[..byte_width];
            match str::from_utf8(utf_slice) {
                Ok(x) => buffer.push_str(x),
                // FIXME: This api should honestly be rewritter from scratch
                Err(_) => panic!("Multiple incorrect bytes instead of one: {utf_slice:?}"),
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
