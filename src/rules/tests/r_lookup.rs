#[cfg(test)]
mod test {
    use crate::{
        rules::tests::helpers::run_integration_engine_test, test_helpers::DefaultResolverTest,
    };

    #[tokio::test]
    async fn test_dns_lookup_success() {
        assert!(run_integration_engine_test(
            DefaultResolverTest,
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
    }

    #[tokio::test]
    async fn test_dns_lookup_failure() {
        assert!(run_integration_engine_test(
            DefaultResolverTest,
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
