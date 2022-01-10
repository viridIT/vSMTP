#[cfg(test)]
pub mod test {
    use crate::{
        rules::tests::helpers::run_integration_engine_test, test_helpers::DefaultResolverTest,
    };

    #[tokio::test]
    async fn test_connect_rules() {
        assert!(run_integration_engine_test(
            DefaultResolverTest {},
            "./src/rules/tests/rules/connect/valid_connect.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            b"",
            b"220 test.server.com Service ready\r\n"
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test(
            DefaultResolverTest {},
            "./src/rules/tests/rules/connect/invalid_connect.vsl",
            "./src/rules/tests/configs/default.config.toml",
            users::mock::MockUsers::with_current_uid(1),
            b"",
            b"",
        )
        .await
        .is_err());
    }
}
