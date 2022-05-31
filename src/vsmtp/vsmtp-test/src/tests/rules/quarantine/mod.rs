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
use vsmtp_common::mail_context::{MailContext, MessageBody};
use vsmtp_common::re::{serde_json, tokio};
use vsmtp_server::message_from_file_path;
use vsmtp_server::ProcessMessage;

#[tokio::test]
#[ignore]
async fn test_quarantine() {
    let mut config = crate::config::local_test();
    config.server.queues.dirpath = "./tmp/tests/rules/quarantine/spool".into();
    config.app.dirpath = "./tmp/tests/rules/quarantine/".into();
    config.app.vsl.filepath = Some("./src/tests/rules/quarantine/main.vsl".into());

    let (delivery_sender, _d) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.delivery.channel_size);

    let (working_sender, _w) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.working.channel_size);

    assert!(test_receiver! {
        on_mail => &mut vsmtp_server::MailHandler::new(working_sender, delivery_sender),
        with_config => config.clone(),
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

    println!("{}", message.display());

    let ctx =
        serde_json::from_str::<MailContext>(&std::fs::read_to_string(&message).unwrap()).unwrap();

    let mut path = config.server.queues.dirpath.clone();
    path.push(format!("mails/{}.eml", ctx.metadata.unwrap().message_id));

    assert_eq!(
        message_from_file_path(path).await.unwrap(),
        MessageBody::Raw(
            ["from: 'abc'", "to: 'def'"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        )
    );
}
