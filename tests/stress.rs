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
    rules::rule_engine,
    server::ServerVSMTP,
    smtp::mail::MailContext,
};

const SERVER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const CLIENT_THREAD_COUNT: u64 = 10;
const MAIL_PER_THREAD: u64 = 100;

fn get_mail() -> lettre::Message {
    lettre::Message::builder()
        .from("NoBody <nobody@domain.tld>".parse().unwrap())
        .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
        .to("Hei <hei@domain.tld>".parse().unwrap())
        .subject("Happy new year")
        .body(String::from("Be happy!"))
        .unwrap()
}

fn send_one_mail(mailer: &lettre::SmtpTransport) {
    let email = get_mail();

    match lettre::Transport::send(mailer, &email) {
        Ok(_) => {}
        Err(e) => panic!("{}", e),
    }
}

async fn run_one_connection() {
    let mailer = lettre::SmtpTransport::builder_dangerous("0.0.0.0")
        .port(10027)
        .build();

    for i in 0..MAIL_PER_THREAD {
        let tracer = opentelemetry::global::tracer("sender");

        let span = opentelemetry::trace::Tracer::start(&tracer, format!("Sending: {}", i));
        let _ =
            <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(
                span,
            );
        send_one_mail(&mailer);
    }
}

async fn send_payload() {
    let mut clients = vec![];

    for client_id in 0..CLIENT_THREAD_COUNT {
        let tracer = opentelemetry::global::tracer("client");
        let span = opentelemetry::trace::Tracer::start(&tracer, format!("connect: {}", client_id));
        let cx =
            <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(
                span,
            );

        clients.push(tokio::spawn(async move {
            opentelemetry::trace::FutureExt::with_context(run_one_connection(), cx).await
        }));
    }

    for i in clients {
        i.await.unwrap();
    }
}

struct Nothing;

#[async_trait::async_trait]
impl vsmtp::resolver::Resolver for Nothing {
    async fn deliver(&mut self, _: &ServerConfig, _: &MailContext) -> anyhow::Result<()> {
        Ok(())
    }
}

#[ignore = "heavy work"]
#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
async fn stress() {
    let config = ServerConfig::builder()
        .with_server(
            "stress.server.com",
            "0.0.0.0:10027".parse().expect("valid address"),
            "0.0.0.0:10589".parse().expect("valid address"),
            "0.0.0.0:10467".parse().expect("valid address"),
            8,
        )
        .with_logging(
            "./tests/generated/output.log",
            vsmtp::collection! {"default".to_string() => log::LevelFilter::Error},
        )
        .without_smtps()
        .with_default_smtp()
        .with_delivery("./tmp/generated/spool", vsmtp::collection! {})
        .with_rules("./tmp/no_rules")
        .with_default_reply_codes()
        .build()
        .unwrap();

    log4rs::init_config(get_logger_config(&config).unwrap()).unwrap();

    rule_engine::init(Box::leak(config.rules.dir.clone().into_boxed_str()))
        .map_err(|error| {
            log::error!("could not initialize the rule engine: {}", error);
            error
        })
        .unwrap();

    let mut server = ServerVSMTP::new(std::sync::Arc::new(config))
        .await
        .expect("failed to initialize server");

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("vsmtp-stress")
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();

    let span = opentelemetry::trace::Tracer::start(&tracer, "root");
    let cx =
        <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(span);

    let listen_and_serve = server
        .with_resolver("default", Nothing {})
        .listen_and_serve();

    tokio::select! {
        server_finished = tokio::time::timeout(SERVER_TIMEOUT, listen_and_serve) => {
            match server_finished {
                Ok(Ok(_)) => unreachable!(),
                Ok(Err(e)) => panic!("{}", e),
                Err(_) => {}
            };
        },
        _ = opentelemetry::trace::FutureExt::with_context(send_payload(), cx) => {
            println!("all client done");
        }
    }
    opentelemetry::global::shutdown_tracer_provider();
}
