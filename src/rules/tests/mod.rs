mod actions;
mod connect;
mod init;
mod object_parsing;
mod users;

#[cfg(test)]
pub mod test {
    use crate::{
        config::server_config::ServerConfig, mailprocessing::mail_receiver::MailReceiver,
        model::mail::MailContext, resolver::DataEndResolver, smtp::code::SMTPReplyCode,
        tests::Mock,
    };

    pub fn get_test_config(path: &str) -> ServerConfig {
        toml::from_str(&std::fs::read_to_string(path).expect("failed to read config from file"))
            .expect("cannot parse config from toml")
    }

    struct DefaultResolverTest;

    #[async_trait::async_trait]
    impl DataEndResolver for DefaultResolverTest {
        async fn on_data_end(
            _: &ServerConfig,
            _: &MailContext,
        ) -> Result<SMTPReplyCode, std::io::Error> {
            Ok(SMTPReplyCode::Code250)
        }
    }

    pub async fn make_test<T: crate::resolver::DataEndResolver>(
        smtp_input: &[u8],
        expected_output: &[u8],
        mut config: ServerConfig,
    ) -> Result<(), std::io::Error> {
        config.prepare();

        let mut receiver = MailReceiver::<T>::new(
            "0.0.0.0:0".parse().unwrap(),
            None,
            std::sync::Arc::new(config),
        );
        let mut write = Vec::new();
        let mock = Mock::new(smtp_input.to_vec(), &mut write);

        match receiver.receive_plain(mock).await {
            Ok(mut mock) => {
                let _ = std::io::Write::flush(&mut mock);
                assert_eq!(
                    std::str::from_utf8(&write),
                    std::str::from_utf8(&expected_output.to_vec())
                );
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
