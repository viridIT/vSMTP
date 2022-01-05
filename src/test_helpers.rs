use crate::{
    config::server_config::ServerConfig, connection::Connection, io_service::IoService,
    model::mail::MailContext, resolver::DataEndResolver, server::handle_client,
    smtp::code::SMTPReplyCode,
};

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

pub struct DefaultResolverTest;

#[async_trait::async_trait]
impl DataEndResolver for DefaultResolverTest {
    async fn on_data_end(
        _: &ServerConfig,
        _: &MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error> {
        Ok(SMTPReplyCode::Code250)
    }
}

pub async fn test_receiver<T: DataEndResolver>(
    smtp_input: &[u8],
    expected_output: &[u8],
    mut config: ServerConfig,
) -> Result<(), std::io::Error> {
    config.prepare();

    let mut written_data = Vec::new();
    let mut mock = Mock::new(smtp_input.to_vec(), &mut written_data);
    let mut io = IoService::new(&mut mock);
    let mut conn = Connection::<Mock<'_>>::from_plain(
        "0.0.0.0:0".parse().unwrap(),
        std::sync::Arc::new(config),
        &mut io,
    )?;

    handle_client::<T, Mock<'_>>(&mut conn, None).await?;
    std::io::Write::flush(&mut conn.io_stream.inner)?;

    assert_eq!(
        std::str::from_utf8(&written_data),
        std::str::from_utf8(&expected_output.to_vec())
    );
    Ok(())
}
