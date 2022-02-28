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

async fn run_one_connection(client_nb: u64) {
    let tracer = opentelemetry::global::tracer("client");
    let span = opentelemetry::trace::Tracer::start(&tracer, format!("Connection: {client_nb}"));
    let cx =
        <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(span);

    let mailer = std::sync::Arc::new(
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous("0.0.0.0")
            .port(10027)
            .build(),
    );

    for i in 0..MAIL_PER_THREAD {
        let sender = mailer.clone();
        opentelemetry::trace::FutureExt::with_context(async move {
            let tracer = opentelemetry::global::tracer("mail");
            let span = opentelemetry::trace::Tracer::start(&tracer, format!("Sending: {i}"));
            let cx =
                <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(span);

            opentelemetry::trace::FutureExt::with_context(
                lettre::AsyncTransport::send(sender.as_ref(), get_mail()),
                cx,
            )
            .await
            .unwrap();
        }, cx.clone()).await;
    }
}

#[ignore = "require the test 'listen_and_serve' and a 'jaeger-all-in-one' to run in background"]
#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
async fn send_payload() {
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("vsmtp-stress")
        .install_batch(opentelemetry::runtime::Tokio)
        .unwrap();

    let span = opentelemetry::trace::Tracer::start(&tracer, "root");
    let cx =
        <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(span);

    opentelemetry::trace::FutureExt::with_context(async move {
        let task = (0..CLIENT_THREAD_COUNT).into_iter().map(|client_nb| {
            let tracer = opentelemetry::global::tracer("sending-payload");
            let span = opentelemetry::trace::Tracer::start(&tracer, "sending payload".to_string());
            let cx =
                <opentelemetry::Context as opentelemetry::trace::TraceContextExt>::current_with_span(span);

            tokio::spawn(opentelemetry::trace::FutureExt::with_context(run_one_connection(client_nb), cx))
        }).collect::<Vec<_>>();

        for i in task {
            i.await.unwrap();
        }
    }, cx).await;
    opentelemetry::global::shutdown_tracer_provider();
}
