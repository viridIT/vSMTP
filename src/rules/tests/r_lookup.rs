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
mod test {
    use crate::{
        receiver::test_helpers::DefaultResolverTest,
        rules::tests::helpers::run_integration_engine_test,
    };

    #[tokio::test]
    async fn test_dns_lookup_success() {
        assert!(run_integration_engine_test(
            "127.0.0.1:0",
            DefaultResolverTest,
            "./src/rules/tests/rules/actions/r_lookup.vsl",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foobar\r\n", "MAIL FROM: <example@localhost>\r\n"]
                .concat()
                .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "250 Ok\r\n"
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_dns_lookup_failure() {
        assert!(run_integration_engine_test(
            "0.0.0.0:0",
            DefaultResolverTest,
            "./src/rules/tests/rules/actions/r_lookup.vsl",
            users::mock::MockUsers::with_current_uid(1),
            ["HELO foobar\r\n", "MAIL FROM: <example@invalid.com>\r\n"]
                .concat()
                .as_bytes(),
            [
                "220 test.server.com Service ready\r\n",
                "250 Ok\r\n",
                "554 permanent problems with the remote server\r\n",
            ]
            .concat()
            .as_bytes(),
        )
        .await
        .is_ok());
    }
}
