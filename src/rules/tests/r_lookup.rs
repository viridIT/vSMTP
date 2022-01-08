#[cfg(test)]
mod test {
    use crate::{
        config::server_config::ServerConfig, model::mail::MailContext, resolver::DataEndResolver,
        rules::tests::helpers::run_integration_engine_test, smtp::code::SMTPReplyCode,
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
    async fn test_reverse_lookup() {
        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/actions/r_lookup.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foobar\r\n",].concat().as_bytes(),
            ["220 test.server.com Service ready\r\n", "250 Ok\r\n",]
                .concat()
                .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test::<Test>(
            "./src/rules/tests/rules/actions/r_lookup_failure.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foobar\r\n",].concat().as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "554 permanent problems with the remote server\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());
    }
}
