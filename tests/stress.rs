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
const CLIENT_THREAD_COUNT: u64 = 1;
const MAIL_PER_THREAD: u64 = 1;

fn get_mail() -> lettre::Message {
    lettre::Message::builder()
        .from("NoBody <nobody@domain.tld>".parse().unwrap())
        .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
        .to("Hei <hei@domain.tld>".parse().unwrap())
        .subject("Happy new year")
        .body(String::from("Be happy!"))
        .unwrap()
}

async fn send_one_mail(mailer: &lettre::AsyncSmtpTransport<lettre::Tokio1Executor>, mail_nb: u64) {
    let tracer = opentelemetry::global::tracer("mail");
    let span = opentelemetry::trace::Tracer::start(&tracer, format!("Sending: {mail_nb}"));
    let cx =
        <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(span);

    match opentelemetry::trace::FutureExt::with_context(
        lettre::AsyncTransport::send(mailer, get_mail()),
        cx,
    )
    .await
    {
        Ok(_) => {}
        Err(e) => panic!("error while sending {e}"),
    }
    log::error!("here1");
}

async fn run_one_connection(client_nb: u64) {
    let tracer = opentelemetry::global::tracer("client");
    let span = opentelemetry::trace::Tracer::start(&tracer, format!("Connecting: {client_nb}"));
    let cx =
        <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(span);

    let mailer = lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous("0.0.0.0")
        .port(10027)
        .build();

    for i in 0..MAIL_PER_THREAD {
        opentelemetry::trace::FutureExt::with_context(send_one_mail(&mailer, i), cx.clone()).await;
    }
    log::error!("here2");
}

async fn send_payload() {
    let tracer = opentelemetry::global::tracer("payload");
    let span = opentelemetry::trace::Tracer::start(&tracer, "sending payload".to_string());
    let cx =
        <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(span);

    for client_nb in 0..CLIENT_THREAD_COUNT {
        opentelemetry::trace::FutureExt::with_context(run_one_connection(client_nb), cx.clone())
            .await;
    }
    log::error!("here3");
}

struct Nothing;

#[async_trait::async_trait]
impl vsmtp::resolver::Resolver for Nothing {
    async fn deliver(&mut self, _: &ServerConfig, _: &MailContext) -> anyhow::Result<()> {
        log::error!("here");
        Ok(())
    }
}

#[ignore = "heavy work"]
#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
async fn stress() {
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
        .with_delivery("./tmp/generated/spool", vsmtp::collection! {})
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

    server.with_resolver("default", Nothing {});

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("vsmtp-stress")
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();

    let span = opentelemetry::trace::Tracer::start(&tracer, "root");
    let cx =
        <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(span);

    let server = tokio::spawn(async move {
        tokio::time::timeout(SERVER_TIMEOUT, server.listen_and_serve()).await
    });

    let client = opentelemetry::trace::FutureExt::with_context(send_payload(), cx);

    tokio::select! {
        server_finished = server => {
            match server_finished {
                Ok(Ok(_)) => unreachable!(),
                Ok(Err(e)) => panic!("{}", e),
                Err(_) => {}
            };
        },
        clients_finished = client => {
            log::error!("all client done {:?}", clients_finished);
        },
    }

    opentelemetry::global::shutdown_tracer_provider();
}
