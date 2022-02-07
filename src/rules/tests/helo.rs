/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
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
#[cfg(test)]
pub mod test {
    use crate::{
        receiver::test_helpers::DefaultResolverTest,
        rules::tests::helpers::run_integration_engine_test,
    };

    #[tokio::test]
    async fn test_valid_helo() {
        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/helo/valid_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO viridit.com\r\n"].concat().as_bytes(),
            ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                .concat()
                .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/helo/valid_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO ibm.com\r\n"].concat().as_bytes(),
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

    #[tokio::test]
    async fn test_types_helo() {
        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/helo/regex_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO viridit.eu\r\n"].concat().as_bytes(),
            ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                .concat()
                .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/helo/regex_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO viridit.com\r\n"].concat().as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "554 permanent problems with the remote server\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/helo/file_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO viridit.fr\r\n"].concat().as_bytes(),
            ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                .concat()
                .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/helo/file_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO green.foo\r\n"].concat().as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "554 permanent problems with the remote server\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());

        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest {},
            "./src/rules/tests/rules/helo/file_helo.vsl",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foo.com\r\n"].concat().as_bytes(),
            ["220 test.server.com Service ready\r\n", "250 Ok\r\n"]
                .concat()
                .as_bytes(),
        )
        .await
        .is_ok());
    }
}
