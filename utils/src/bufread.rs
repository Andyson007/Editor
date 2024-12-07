use core::str;
use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, BufReader};

pub trait BufReaderExt {
    /// Returns none if the buffer was read to completion
    fn read_valid_str(
        &mut self,
        buffer: &mut String,
    ) -> impl std::future::Future<Output = io::Result<Option<u8>>> + Send;
}

impl<T> BufReaderExt for BufReader<T>
where
    T: AsyncRead + Unpin + Send,
{
    async fn read_valid_str(&mut self, buffer: &mut String) -> io::Result<Option<u8>> {
        loop {
            let first = self.read_u8().await?;
            let leading = first.leading_ones() as usize;
            if leading > 4 {
                return Ok(None);
            }
            let mut buf = [0; 4];
            buf[0] = first;
            for x in buf.iter_mut().take(leading).skip(1) {
                *x = self.read_u8().await?;
            }
            match str::from_utf8(&buf[..leading]) {
                Ok(x) => buffer.push_str(x),
                Err(_) => return Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn all_valid() {}
}
