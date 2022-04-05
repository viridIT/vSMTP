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
use vsmtp_common::re::anyhow;
use vsmtp_config::{get_log4rs_config, re::log4rs, Config};
use vsmtp_rule_engine::rule_engine::RuleEngine;
use vsmtp_server::{ProcessMessage, Server};

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

    let mut config = Config::builder()
        .with_version_str("<1.0.0")
        .unwrap()
        .with_server_name_and_client_count("stress.server.com", STRESS_CONFIG.client_count_max)
        .with_default_system()
        .with_interfaces(
            &["0.0.0.0:10027".parse().expect("valid")],
            &["0.0.0.0:10589".parse().expect("valid")],
            &["0.0.0.0:10467".parse().expect("valid")],
        )
        .with_default_logs_settings()
        .with_spool_dir_and_default_queues("./tmp/stress/spool")
        .without_tls_support()
        .with_default_smtp_options()
        .with_default_smtp_error_handler()
        .with_default_smtp_codes()
        .without_auth()
        .with_app_at_location("./tmp/stress")
        .with_vsl("./tests/stress/main.vsl")
        .with_app_logs("./tmp/stress/app.log")
        .without_services()
        .with_system_dns()
        .validate()
        .unwrap();

    config.server.queues.working.channel_size = 1;
    config.server.queues.delivery.channel_size = 1;

    get_log4rs_config(&config, true)
        .context("Logs configuration contain error")
        .map(log4rs::init_config)
        .context("Cannot initialize logs")
        .unwrap()
        .unwrap();

    let sockets = (
        std::net::TcpListener::bind(&config.server.interfaces.addr[..]).unwrap(),
        std::net::TcpListener::bind(&config.server.interfaces.addr_submission[..]).unwrap(),
        std::net::TcpListener::bind(&config.server.interfaces.addr_submissions[..]).unwrap(),
    );

    let (delivery_sender, _delivery_receiver) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.delivery.channel_size);

    let (working_sender, _working_receiver) =
        tokio::sync::mpsc::channel::<ProcessMessage>(config.server.queues.working.channel_size);

    let rule_engine = std::sync::Arc::new(std::sync::RwLock::new(RuleEngine::new(&None).unwrap()));

    let mut server = Server::new(
        std::sync::Arc::new(config),
        sockets,
        rule_engine,
        working_sender,
        delivery_sender,
    )
    .unwrap();

    tokio::time::timeout(SERVER_TIMEOUT, server.listen_and_serve())
        .await
        .unwrap()
        .unwrap();
}
