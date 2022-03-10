/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/

#[derive(Debug)]
pub enum ReadError {
    Eof,
    Blocking,
    Other(std::io::Error),
}

/// This class provide an abstraction of an IO stream
/// providing data for the receiver
pub struct IoService<'a, T>
where
    T: std::io::Read + std::io::Write,
{
    /// inner stream
    pub inner: &'a mut T,
    // buffer used by AsyncBufRead impl
    buffer: Vec<u8>,
}

impl<'a, T> std::io::Read for IoService<'a, T>
where
    T: std::io::Read + std::io::Write,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<'a, T> std::io::Write for IoService<'a, T>
where
    T: std::io::Read + std::io::Write,
{
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.inner.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, T> tokio::io::AsyncRead for IoService<'a, T>
where
    T: std::io::Read + std::io::Write,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let x = self.as_mut().inner.read(unsafe {
            &mut *(buf.unfilled_mut() as *mut [std::mem::MaybeUninit<u8>] as *mut [u8])
        });
        match x {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
            Ok(i) => {
                buf.set_filled(i);
                std::task::Poll::Ready(Ok(()))
            }
            Err(e) => std::task::Poll::Ready(Err(e)),
        }
    }
}

impl<'a, T> tokio::io::AsyncBufRead for IoService<'a, T>
where
    T: std::io::Read + std::io::Write,
{
    fn poll_fill_buf(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        self.get_mut().poll_fill_buf_priv(cx)
    }

    fn consume(self: std::pin::Pin<&mut Self>, amt: usize) {
        self.get_mut().buffer = self.buffer[amt..].to_vec();
    }
}

impl<'a, T: std::io::Read + std::io::Write> IoService<'a, T> {
    ///
    pub fn new(inner: &'a mut T) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
        }
    }

    fn poll_fill_buf_priv(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<&[u8]>> {
        if self.buffer.is_empty() {
            let mut raw = vec![0; 1000];
            let mut buf = tokio::io::ReadBuf::new(&mut raw);
            match tokio::io::AsyncRead::poll_read(std::pin::Pin::new(self), cx, &mut buf) {
                std::task::Poll::Pending => return std::task::Poll::Pending,
                std::task::Poll::Ready(Ok(_)) => self.buffer = buf.filled().to_vec(),
                std::task::Poll::Ready(Err(e)) => return std::task::Poll::Ready(Err(e)),
            };
        }
        std::task::Poll::Ready(Ok(&self.buffer))
    }

    fn remove_new_line(mut buf: &str) -> String {
        if buf.ends_with('\n') {
            buf = &buf[..buf.len() - 1];
            if buf.ends_with('\r') {
                buf = &buf[..buf.len() - 1];
            }
        }

        buf.to_string()
    }

    /// Read one line from the inner stream
    ///
    /// # Errors
    ///
    /// * Eof if read size is 0
    /// * Blocking if would block
    /// * Other (io error)
    pub async fn get_next_line_async(&mut self) -> Result<String, ReadError> {
        let mut buf = String::new();
        match tokio::io::AsyncBufReadExt::read_line(self, &mut buf).await {
            Ok(0) => Err(ReadError::Eof),
            Ok(size) => Ok(Self::remove_new_line(&buf[..size])),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Err(ReadError::Blocking),
            Err(e) => Err(ReadError::Other(e)),
        }
    }
}
