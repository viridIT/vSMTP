#[cfg(test)]
pub mod test {
    use crate::{
        config::server_config::ServerConfig,
        model::mail::MailContext,
        resolver::DataEndResolver,
        rules::{
            tests::init::run_engine_test,
            tests::test::{get_test_config, make_test},
        },
        smtp::code::SMTPReplyCode,
    };

    struct Test;

    #[async_trait::async_trait]
    impl DataEndResolver for Test {
        async fn on_data_end(
            _: &ServerConfig,
            _: &MailContext,
        ) -> Result<SMTPReplyCode, std::io::Error> {
            Ok(SMTPReplyCode::Code250)
        }
    }

    #[tokio::test]
    async fn test_connect_rules() {
        async fn test() {
            assert!(make_test::<Test>(
                b"",
                ["220 test.server.com Service ready\r\n",]
                    .concat()
                    .as_bytes(),
                get_test_config("./src/rules/tests/configs/default.config.toml"),
            )
            .await
            .is_ok());
        }

        run_engine_test(
            "./src/rules/tests/rules/connect/valid_connect.vsl",
            users::mock::MockUsers::with_current_uid(1),
            test,
        )
        .await;

        async fn test2() {
            assert!(make_test::<Test>(
                b"",
                ["220 test.server.com Service ready\r\n",]
                    .concat()
                    .as_bytes(),
                get_test_config("./src/rules/tests/configs/default.config.toml"),
            )
            .await
            .is_err());
        }

        run_engine_test(
            "./src/rules/tests/rules/connect/invalid_connect.vsl",
            users::mock::MockUsers::with_current_uid(1),
            test2,
        )
        .await;
    }
}
