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
use std::thread;

use vsmtp::{config::server_config::ServerConfig, server::ServerVSMTP};

const SERVER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const CLIENT_THREAD_COUNT: u64 = 1000;
const MAIL_PER_THREAD: u64 = 1000;

#[ignore = "too heavy"]
#[tokio::test]
#[cfg(false)]
async fn test_dos() {
    match fork::fork().expect("failed to fork process") {
        fork::Fork::Parent(_) => {
            let mut config: ServerConfig = toml::from_str(include_str!("dos.config.toml"))
                .expect("cannot parse config from toml");
            config.prepare();
            let config = std::sync::Arc::new(config);

            let mut server = ServerVSMTP::new(config)
                .await
                .expect("failed to initialize server");

            log::warn!("Listening on: {:?}", server.addr());
            match tokio::time::timeout(SERVER_TIMEOUT, server.listen_and_serve()).await {
                Ok(Ok(_)) => unreachable!(),
                Ok(Err(e)) => panic!("{}", e),
                Err(_) => {}
            };
        }
        fork::Fork::Child => {
            for tid in 0..CLIENT_THREAD_COUNT {
                thread::spawn(move || {
                    let mailer = lettre::SmtpTransport::builder_dangerous("0.0.0.0")
                        .port(10027)
                        .build();

                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("./tests/generated/failed.log")
                        .unwrap();

                    let mut rng = rand::thread_rng();

                    for i in 0..MAIL_PER_THREAD {
                        let email = lettre::Message::builder()
                            .from("NoBody <nobody@domain.tld>".parse().unwrap())
                            .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
                            .to("Hei <hei@domain.tld>".parse().unwrap())
                            .subject(format!("DOS {}/{}", i, tid))
                            .body(
                                (0..rand::Rng::gen::<u16>(&mut rng))
                                    .map(|_| rand::Rng::gen::<u8>(&mut rng))
                                    .collect::<Vec<_>>(),
                            )
                            .unwrap();

                        match lettre::Transport::send(&mailer, &email) {
                            Ok(_) => {}
                            Err(e) => {
                                std::io::Write::write_fmt(
                                    &mut file,
                                    format_args!("{}\treason = {}\n", i, e),
                                )
                                .unwrap();
                            }
                        }
                    }
                });
            }
        }
    };
}
