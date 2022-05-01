use std::str::FromStr;

/*
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
*/
use crate::{config, test_receiver};
use vsmtp_server::re::tokio;

#[tokio::test]
async fn info_message() {
    let mut config = config::local_test();
    config.app.vsl.filepath = std::path::PathBuf::from_str("./src/tests/custom_codes.vsl").unwrap();

    assert!(test_receiver! {
        with_config => config,
        [
            "HELO someone\r\n",
            "HELO foo.bar\r\n",
            "MAIL FROM:<a@satan.org>\r\n",
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 cannot identify with 'someone'.\r\n",
            "250 Ok\r\n",
            "501 4.7.1 satan is blacklisted on this server\r\n",
        ]
        .concat()
    }
    .is_ok());
}
