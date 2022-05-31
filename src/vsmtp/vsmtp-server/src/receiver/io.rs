/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
///
#[derive(Debug)]
pub struct AbstractIO<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    ///
    pub inner: S,
    buf: Vec<u8>,
}

macro_rules! ready {
    ($e:expr) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => {
                return std::task::Poll::Pending;
            }
        }
    };
}

impl<S> tokio::io::AsyncRead for AbstractIO<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

const BUFFER_SIZE: usize = 1000;
const NEEDLE: &[u8] = b"\r\n";

impl<S> AbstractIO<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    ///
    #[allow(clippy::missing_const_for_fn)] // unstable for msrv
    pub fn new(stream: S) -> Self {
        Self {
            inner: stream,
            buf: Vec::new(),
        }
    }

    /// Returns the next line from the inner stream. Or [`None`] if stream is closed.
    ///
    /// # Errors
    ///
    /// * timed-out
    /// * [`self`] failed to read
    pub async fn next_line(
        &mut self,
        timeout: Option<std::time::Duration>,
    ) -> std::io::Result<Option<String>> {
        tokio::time::timeout(
            timeout.unwrap_or(std::time::Duration::from_millis(500)),
            self,
        )
        .await
        .map_err(|t| std::io::Error::new(std::io::ErrorKind::TimedOut, t))?
    }
}

impl<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin> tokio::io::AsyncBufRead
    for AbstractIO<S>
{
    fn poll_fill_buf(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        if self.as_mut().buf.is_empty() {
            let mut raw = vec![0; BUFFER_SIZE];
            let mut buf = tokio::io::ReadBuf::new(&mut raw);
            ready!(tokio::io::AsyncRead::poll_read(self.as_mut(), cx, &mut buf))?;
            self.as_mut().buf = buf.filled().to_vec();
        }
        std::task::Poll::Ready(Ok(&self.get_mut().buf))
    }

    fn consume(mut self: std::pin::Pin<&mut Self>, amt: usize) {
        self.buf = self.buf[amt..].to_vec();
    }
}

impl<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin> std::future::Future
    for AbstractIO<S>
{
    type Output = std::io::Result<Option<String>>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        fn to_output(s: Vec<u8>, last: bool) -> std::io::Result<Option<String>> {
            if s.is_empty() {
                Ok(if last { None } else { Some(String::default()) })
            } else {
                Ok(Some(String::from_utf8(s).map_err(|e| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, e)
                })?))
            }
        }

        let mut output = vec![];
        loop {
            let available = ready!(tokio::io::AsyncBufRead::poll_fill_buf(self.as_mut(), cx))?;
            if available.is_empty() {
                return std::task::Poll::Ready(to_output(output, true));
            }

            if let Some(i) = available
                .windows(NEEDLE.len())
                .position(|window| window == NEEDLE)
            {
                let slice = &available[..i];
                output.extend_from_slice(slice);
                tokio::io::AsyncBufReadExt::consume(&mut self.as_mut(), i + NEEDLE.len());
                return std::task::Poll::Ready(to_output(output, false));
            }
            let len = available.len();
            output.extend_from_slice(available);
            tokio::io::AsyncBufReadExt::consume(&mut self.as_mut(), len);
        }
    }
}

#[cfg(test)]
mod tests {
    use vsmtp_test::receiver::Mock;

    use super::*;

    #[tokio::test]
    async fn read() {
        let input = ["a\r\n", "b\r\n", "c\r\n", "d\r\n", "e\r\n", "f\r\n"]
            .concat()
            .as_bytes()
            .to_vec();
        let mut written = Vec::new();
        let mut io = AbstractIO::new(Mock::new(input.clone(), &mut written));

        let mut has_been_read = vec![];

        while let Ok(Some(line)) = io.next_line(None).await {
            has_been_read.push(line);
        }

        pretty_assertions::assert_eq!(
            ["a", "b", "c", "d", "e", "f"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>(),
            has_been_read
        );
    }

    #[tokio::test]
    async fn read_empty_line() {
        let input = ["a\r\n", "\r\n", "c\r\n", "d\r\n", "\r\n", "f\r\n"]
            .concat()
            .as_bytes()
            .to_vec();
        let mut written = Vec::new();
        let mut io = AbstractIO::new(Mock::new(input.clone(), &mut written));

        let mut has_been_read = vec![];

        while let Ok(Some(line)) = io.next_line(None).await {
            has_been_read.push(line);
        }

        pretty_assertions::assert_eq!(
            ["a", "", "c", "d", "", "f"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>(),
            has_been_read
        );
    }

    #[tokio::test]
    async fn read_no_final_crlf() {
        let input = ["a\r\n", "b\r\n", "c\r\n", "d\r\n", "e\r\n", "f"]
            .concat()
            .as_bytes()
            .to_vec();
        let mut written = Vec::new();
        let mut io = AbstractIO::new(Mock::new(input.clone(), &mut written));

        let mut has_been_read = vec![];

        while let Ok(Some(line)) = io.next_line(None).await {
            has_been_read.push(line);
        }
        pretty_assertions::assert_eq!(
            ["a\r\n", "b\r\n", "c\r\n", "d\r\n", "e\r\n", "f"]
                .into_iter()
                .map(str::to_string)
                .collect::<String>(),
            has_been_read.join("\r\n"),
        );
    }

    #[tokio::test]
    async fn read_non_utf8() {
        let input = b"\xc3\x28".to_vec();
        std::str::from_utf8(&input).unwrap_err();

        let mut written = Vec::new();
        let mut io = AbstractIO::new(Mock::new(input.clone(), &mut written));

        assert_eq!(
            io.next_line(None).await.unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
    }
}
