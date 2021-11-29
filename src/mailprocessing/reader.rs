/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
pub enum ReadError {
    Eof,
    Blocking,
    Other(std::io::Error),
}

pub struct MyConnectionIO<'a, T>
where
    T: std::io::Read + std::io::Write,
{
    pub inner: &'a mut T,
    buffer: Vec<u8>,
}

impl<'a, T> std::io::Read for MyConnectionIO<'a, T>
where
    T: std::io::Read + std::io::Write,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<'a, T> std::io::BufRead for MyConnectionIO<'a, T>
where
    T: std::io::Read + std::io::Write,
{
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        let mut buf = vec![0; 1000];
        if self.buffer.is_empty() {
            match std::io::Read::read(self, &mut buf) {
                Ok(size) => self.buffer = buf[..size].to_vec(),
                Err(e) => return Err(e),
            };
        }
        Ok(&self.buffer)
    }

    fn consume(&mut self, amt: usize) {
        self.buffer = self.buffer[amt..].to_vec();
    }
}

impl<'a, T: std::io::Read + std::io::Write> MyConnectionIO<'a, T> {
    pub fn new(inner: &'a mut T) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
        }
    }

    pub fn get_next_line(&mut self) -> Result<String, ReadError> {
        let mut buf = String::new();
        match std::io::BufRead::read_line(self, &mut buf) {
            Ok(0) => Err(ReadError::Eof),
            Ok(size) => {
                let mut out = &buf[..size];
                if out.ends_with('\n') {
                    out = &out[..out.len() - 1];
                    if out.ends_with('\r') {
                        out = &out[..out.len() - 1];
                    }
                }

                Ok(out.to_string())
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Err(ReadError::Blocking),
            Err(e) => Err(ReadError::Other(e)),
        }
    }

    pub fn write_to_stream(&mut self, buf: &str) -> Result<(), std::io::Error>
    where
        T: std::io::Write,
    {
        match self.inner.write_all(buf.as_bytes()) {
            Ok(_) => Ok(()),
            Err(e) => {
                log::error!(
                    target: "mail_receiver",
                    "Error on sending response (receiving); error = {:?}", e
                );
                Err(e)
            }
        }
    }
}
