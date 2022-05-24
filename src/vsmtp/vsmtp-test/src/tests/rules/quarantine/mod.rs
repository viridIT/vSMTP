/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/

use crate::test_receiver;
use vsmtp_common::mail_context::{Body, MailContext};
use vsmtp_server::re::tokio;
use vsmtp_server::ProcessMessage;

#[tokio::test]
async fn test_quarantine() {
    let mut config = crate::config::local_test();
    config.app.dirpath = "./tmp/tests/rules/quarantine/".into();
    config.app.vsl.filepath = Some("./src/tests/rules/quarantine/main.vsl".into());

    let (delivery_sender, _d) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.delivery.channel_size);

    let (working_sender, _w) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.working.channel_size);

    assert!(test_receiver! {
        on_mail => &mut vsmtp_server::MailHandler { working_sender, delivery_sender },
        with_config => config,
        [
            "HELO foobar\r\n",
            "MAIL FROM:<john.doe@example.com>\r\n",
            "RCPT TO:<aa@bb>\r\n",
            "DATA\r\n",
            "from: 'abc'\r\n",
            "to: 'def'\r\n",
            ".\r\n",
            "QUIT\r\n",
        ]
        .concat(),
        [
            "220 testserver.com Service ready\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "250 Ok\r\n",
            "354 Start mail input; end with <CRLF>.<CRLF>\r\n",
            "250 Ok\r\n",
            "221 Service closing transmission channel\r\n",
        ]
        .concat()
    }
    .is_ok());

    let message = std::fs::read_dir("./tmp/tests/rules/quarantine/john/")
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();

    assert_eq!(
        MailContext::from_file(&message).unwrap().body,
        Body::Raw("from: 'abc'\nto: 'def'\n".to_string())
    );

    std::fs::remove_file(message).unwrap();
}
