pub mod config;
pub mod mailprocessing;
pub mod model;
pub mod resolver;
pub mod rules;
pub mod server;
pub mod smtp;

#[macro_export]
macro_rules! collection {
    // map-like
    ($($k:expr => $v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        Iterator::collect(IntoIterator::into_iter([$(($k, $v),)*]))
    }};
    // set-like
    ($($v:expr),* $(,)?) => {{
        use std::iter::{Iterator, IntoIterator};
        Iterator::collect(IntoIterator::into_iter([$($v,)*]))
    }};
}

pub mod tests {

    pub struct Mock<'a> {
        read_cursor: std::io::Cursor<Vec<u8>>,
        write_cursor: std::io::Cursor<&'a mut Vec<u8>>,
    }

    impl<'a> Mock<'a> {
        pub fn new(read: Vec<u8>, write: &'a mut Vec<u8>) -> Self {
            Self {
                read_cursor: std::io::Cursor::new(read),
                write_cursor: std::io::Cursor::new(write),
            }
        }
    }

    impl std::io::Write for Mock<'_> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.write_cursor.write(buf)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.write_cursor.flush()
        }
    }

    impl std::io::Read for Mock<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.read_cursor.read(buf)
        }
    }
}
