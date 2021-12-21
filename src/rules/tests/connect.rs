#[cfg(test)]
pub mod test {
    use crate::{
        config::server_config::ServerConfig,
        model::mail::MailContext,
        resolver::DataEndResolver,
        rules::{
            address::Address,
            tests::init::run_engine_test,
            tests::test::{get_test_config, make_test},
        },
        smtp::code::SMTPReplyCode,
    };
    use std::collections::HashSet;

    #[tokio::test]
    async fn test_connect_rules() {
        struct Test;

        #[async_trait::async_trait]
        impl DataEndResolver for Test {
            async fn on_data_end(
                _: &ServerConfig,
                ctx: &MailContext,
            ) -> Result<SMTPReplyCode, std::io::Error> {
                assert_eq!(ctx.envelop.helo, "foobar");
                assert_eq!(ctx.envelop.mail_from.full(), "john@doe");
                assert_eq!(
                    ctx.envelop.rcpt,
                    HashSet::from([Address::new("aa@bb").unwrap()])
                );
                assert_eq!(ctx.body, "");

                Ok(SMTPReplyCode::Code250)
            }
        }

        async fn test() {
            // TODO: replace with just the expected end result.
            assert!(make_test::<Test>(
                [
                    "HELO foobar\r\n",
                    "MAIL FROM:<john@doe>\r\n",
                    "RCPT TO:<aa@bb>\r\n",
                    "DATA\r\n",
                    ".\r\n",
                    "QUIT\r\n",
                ]
                .concat()
                .as_bytes(),
                [
                    "220 test.server.com Service ready\r\n",
                    "250 Ok\r\n",
                    "250 Ok\r\n",
                    "250 Ok\r\n",
                    "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
                    "250 Ok\r\n",
                    "221 Service closing transmission channel\r\n",
                ]
                .concat()
                .as_bytes(),
                get_test_config("./src/rules/tests/configs/connect.config.toml"),
            )
            .await
            .is_ok());
        }

        run_engine_test(
            "./src/rules/tests/rules/connect/connect.vsl",
            users::mock::MockUsers::with_current_uid(1),
            test,
        )
        .await;
    }
}
