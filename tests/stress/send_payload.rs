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
use opentelemetry::{global, runtime, trace, Context};

#[derive(Debug, serde::Deserialize)]
struct StressConfig {
    server_ip: String,
    server_port: u16,
    total_client_count: u64,
    mail_per_client: u64,
}

lazy_static::lazy_static! {
    static ref STRESS_CONFIG: StressConfig = {
         std::fs::read_to_string("./tests/stress/send_payload_config.json")
            .map(|str| serde_json::from_str(&str)).unwrap().unwrap()
    };
}

fn get_mail() -> lettre::Message {
    lettre::Message::builder()
        .from("NoBody <nobody@domain.tld>".parse().unwrap())
        .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
        .to("Hei <hei@domain.tld>".parse().unwrap())
        .subject("Happy new year")
        .body(String::from("Be happy!"))
        .unwrap()
}

async fn run_one_connection(client_nb: u64) -> Result<(), u64> {
    let tracer = global::tracer("client");
    let span = trace::Tracer::start(&tracer, format!("Connection: {client_nb}"));
    let cx = <Context as trace::TraceContextExt>::current_with_span(span);

    let mailer = std::sync::Arc::new(
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous(
            STRESS_CONFIG.server_ip.clone(),
        )
        .port(STRESS_CONFIG.server_port)
        // TODO:
        // .tls()
        // .credentials()
        .build(),
    );

    for i in 0..STRESS_CONFIG.mail_per_client {
        let sender = mailer.clone();

        let x = trace::FutureExt::with_context(
            async move {
                let tracer = global::tracer("mail");
                let span = trace::Tracer::start(&tracer, format!("Sending: {i}"));
                let cx = <Context as trace::TraceContextExt>::current_with_span(span);

                trace::FutureExt::with_context(
                    lettre::AsyncTransport::send(sender.as_ref(), get_mail()),
                    cx,
                )
                .await
            },
            cx.clone(),
        )
        .await;

        if x.is_err() {
            return Err(client_nb);
        }
    }

    Ok(())
}

fn create_task(id: u64) -> tokio::task::JoinHandle<std::result::Result<(), u64>> {
    let tracer = global::tracer("register-task");
    let span = trace::Tracer::start(&tracer, format!("Register Task: {id}"));
    let cx = <Context as trace::TraceContextExt>::current_with_span(span);

    tokio::spawn(trace::FutureExt::with_context(run_one_connection(id), cx))
}

#[ignore = "require the test 'listen_and_serve' and a 'jaeger-all-in-one' to run in background"]
#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
async fn send_payload() {
    println!("{:?}", *STRESS_CONFIG);

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("vsmtp-stress")
        .install_batch(runtime::Tokio)
        .unwrap();

    let span = trace::Tracer::start(&tracer, "root");
    let cx = <Context as trace::TraceContextExt>::current_with_span(span);

    trace::FutureExt::with_context(
        async move {
            let mut task = (0..STRESS_CONFIG.total_client_count)
                .into_iter()
                .map(create_task)
                .collect::<Vec<_>>();

            while !task.is_empty() {
                let mut new_task = vec![];
                for i in task {
                    if let Err(id) = i.await.unwrap() {
                        new_task.push(create_task(id + 1000))
                    }
                }
                task = new_task;
            }
        },
        cx,
    )
    .await;

    global::shutdown_tracer_provider();
}
