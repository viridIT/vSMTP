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
use anyhow::Context;
use vsmtp_common::{collection, mail_context::MailContext};
use vsmtp_config::{get_logger_config, ServerConfig};
use vsmtp_server::{resolver::Resolver, server::ServerVSMTP};

#[derive(Debug, serde::Deserialize)]
struct StressConfig {
    client_count_max: i64,
}

lazy_static::lazy_static! {
    static ref STRESS_CONFIG: StressConfig = {
         std::fs::read_to_string("./tests/stress/send_payload_config.json")
            .map(|str| serde_json::from_str(&str)).unwrap().unwrap()
    };
}

const SERVER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

#[ignore = "heavy work"]
#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
async fn listen_and_serve() {
    println!("{:?}", *STRESS_CONFIG);

    let mut config = ServerConfig::builder()
        .with_version_str(">=0.9.0")
        .unwrap()
        .with_server(
            "stress.server.com",
            "root",
            "root",
            "0.0.0.0:10027".parse().expect("valid address"),
            "0.0.0.0:10589".parse().expect("valid address"),
            "0.0.0.0:10467".parse().expect("valid address"),
            8,
        )
        .with_logging(
            "./tmp/tests/stress/output.log",
            collection! {"default".to_string() => log::LevelFilter::Info},
        )
        .without_smtps()
        .with_default_smtp()
        .with_delivery("./tmp/tests/stress/spool")
        .with_rules("./tests/stress/main.vsl", vec![])
        .with_default_reply_codes()
        .build()
        .unwrap();

    config.rules.logs.file = "./tmp/tests/stress/app.log".into();
    config.smtp.client_count_max = STRESS_CONFIG.client_count_max;
    config.delivery.queues.working.capacity = 1;
    config.delivery.queues.deliver.capacity = 1;
    config.delivery.queues.deferred.capacity = 1;

    get_logger_config(&config, true)
        .context("Logs configuration contain error")
        .map(log4rs::init_config)
        .context("Cannot initialize logs")
        .unwrap()
        .unwrap();

    let sockets = (
        std::net::TcpListener::bind(&config.server.addr[..]).unwrap(),
        std::net::TcpListener::bind(&config.server.addr_submission[..]).unwrap(),
        std::net::TcpListener::bind(&config.server.addr_submissions[..]).unwrap(),
    );

    let mut server = ServerVSMTP::new(std::sync::Arc::new(config), sockets).unwrap();

    struct Nothing;

    #[async_trait::async_trait]
    impl Resolver for Nothing {
        async fn deliver(&mut self, _: &ServerConfig, _: &MailContext) -> anyhow::Result<()> {
            Ok(())
        }
    }

    server.with_resolver("default", Nothing {});

    tokio::time::timeout(SERVER_TIMEOUT, server.listen_and_serve())
        .await
        .unwrap()
        .unwrap();
}
