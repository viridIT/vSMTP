#[cfg(test)]
pub mod test {
    use crate::{
        rules::tests::helpers::run_integration_engine_test, test_helpers::DefaultResolverTest,
    };

    #[tokio::test]
    async fn test_valid_connect_rules() {
        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/connect/valid_connect.vsl",
            users::mock::MockUsers::with_current_uid(1),
            b"",
            b"220 test.server.com Service ready\r\n",
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_invalid_connect_rules() {
        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/connect/invalid_connect.vsl",
            users::mock::MockUsers::with_current_uid(1),
            b"",
            b"",
        )
        .await
        .is_err());
    }
}
