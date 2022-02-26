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
use vsmtp::{
    config::{get_logger_config, server_config::ServerConfig},
    server::ServerVSMTP,
    smtp::mail::MailContext,
};

const SERVER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[ignore = "heavy work"]
#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
async fn listen_and_serve() {
    let config = ServerConfig::builder()
        .with_server(
            "stress.server.com",
            "foo",
            "foo",
            "0.0.0.0:10027".parse().expect("valid address"),
            "0.0.0.0:10589".parse().expect("valid address"),
            "0.0.0.0:10467".parse().expect("valid address"),
            8,
        )
        .with_logging(
            "./tests/generated/output.log",
            vsmtp::collection! {"default".to_string() => log::LevelFilter::Debug},
        )
        .without_smtps()
        .with_default_smtp()
        .with_delivery("./generated/spool", vsmtp::collection! {})
        .with_rules("./tmp/no_rules", vec![])
        .with_default_reply_codes()
        .build()
        .unwrap();

    log4rs::init_config(get_logger_config(&config).unwrap()).unwrap();

    let sockets = (
        std::net::TcpListener::bind(config.server.addr).unwrap(),
        std::net::TcpListener::bind(config.server.addr_submission).unwrap(),
        std::net::TcpListener::bind(config.server.addr_submissions).unwrap(),
    );

    let mut server = ServerVSMTP::new(std::sync::Arc::new(config), sockets)
        .expect("failed to initialize server");

    struct Nothing;

    #[async_trait::async_trait]
    impl vsmtp::resolver::Resolver for Nothing {
        async fn deliver(&mut self, _: &ServerConfig, _: &MailContext) -> anyhow::Result<()> {
            log::error!("here");
            Ok(())
        }
    }

    server.with_resolver("default", Nothing {});

    tokio::time::timeout(SERVER_TIMEOUT, server.listen_and_serve())
        .await
        .unwrap()
        .unwrap();
}
