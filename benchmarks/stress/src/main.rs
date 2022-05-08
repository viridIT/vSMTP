use lettre::transport::smtp::{
    authentication::{Credentials, Mechanism},
    client::{Tls, TlsParameters},
};

#[cfg(feature = "telemetry")]
use opentelemetry::{
    global, runtime,
    trace::{self, FutureExt},
    Context,
};

fn get_mail() -> lettre::Message {
    lettre::Message::builder()
        .from("NoBody <nobody@domain.tld>".parse().unwrap())
        .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
        .to("Hei <hei@domain.tld>".parse().unwrap())
        .subject("Happy new year")
        .body(String::from("Be happy!"))
        .unwrap()
}

struct StressConfig {
    server_ip: String,
    port_relay: u16,
    port_submission: u16,
    port_submissions: u16,
    total_client_count: u64,
    mail_per_client: u64,
}

const USER_DB: [(&str, &str); 5] = [
    ("stress1", "abc"),
    ("stress2", "bcd"),
    ("stress3", "cde"),
    ("stress4", "efh"),
    ("stress5", "fhi"),
];

async fn run_one_connection(
    config: std::sync::Arc<StressConfig>,
    client_nb: u64,
) -> Result<(), u64> {
    #[cfg(feature = "telemetry")]
    let tracer = global::tracer("client");
    #[cfg(feature = "telemetry")]
    let span = trace::Tracer::start(&tracer, format!("Connection: {client_nb}"));
    #[cfg(feature = "telemetry")]
    let cx = <Context as trace::TraceContextExt>::current_with_span(span);

    let params = TlsParameters::builder("stressserver.com".to_string())
        .dangerous_accept_invalid_certs(true)
        .build()
        .unwrap();

    let tls: i8 = rand::random::<i8>().rem_euclid(4);
    let port = match tls {
        3 => config.port_submissions,
        _ => {
            if rand::random::<bool>() {
                config.port_submission
            } else {
                config.port_relay
            }
        }
    };
    let tls = match tls {
        0 => Tls::None,
        1 => Tls::Opportunistic(params),
        2 => Tls::Required(params),
        3 => Tls::Wrapper(params),
        x => panic!("{x} not handled in range"),
    };

    let mut mailer_builder =
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous(
            config.server_ip.clone(),
        )
        .port(port)
        .tls(tls);

    if rand::random::<bool>() {
        let credentials = USER_DB
            .iter()
            .nth(rand::random::<usize>().rem_euclid(USER_DB.len()))
            .unwrap();

        mailer_builder = mailer_builder
            .authentication(vec![if rand::random::<bool>() {
                Mechanism::Plain
            } else {
                Mechanism::Login
            }])
            .credentials(Credentials::from(*credentials));
    }

    let mailer = std::sync::Arc::new(mailer_builder.build());

    for _i in 0..config.mail_per_client {
        let sender = mailer.clone();

        let future = async move {
            #[cfg(feature = "telemetry")]
            let tracer = global::tracer("mail");
            #[cfg(feature = "telemetry")]
            let span = trace::Tracer::start(&tracer, format!("Sending: {_i}"));
            #[cfg(feature = "telemetry")]
            let cx = <Context as trace::TraceContextExt>::current_with_span(span);

            let future = lettre::AsyncTransport::send(sender.as_ref(), get_mail());
            #[cfg(feature = "telemetry")]
            let future = future.with_context(cx);

            future.await
        };

        #[cfg(feature = "telemetry")]
        let future = future.with_context(cx.clone());

        if let Err(e) = future.await {
            if format!("{:?}", e)
                == r#"lettre::transport::smtp::Error { kind: Connection, source: Os { code: 111, kind: ConnectionRefused, message: "Connection refused" } }"#
            {
                return Ok(());
            }
            return Err(client_nb);
        }
    }

    Ok(())
}

fn create_task(
    config: std::sync::Arc<StressConfig>,
    id: u64,
) -> tokio::task::JoinHandle<std::result::Result<(), u64>> {
    #[cfg(feature = "telemetry")]
    let tracer = global::tracer("register-task");
    #[cfg(feature = "telemetry")]
    let span = trace::Tracer::start(&tracer, format!("Register Task: {id}"));
    #[cfg(feature = "telemetry")]
    let cx = <Context as trace::TraceContextExt>::current_with_span(span);

    let future = run_one_connection(config, id);

    #[cfg(feature = "telemetry")]
    let future = future.with_context(cx);

    tokio::spawn(future)
}

async fn run_stress(config: std::sync::Arc<StressConfig>) {
    #[cfg(feature = "telemetry")]
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("vsmtp-stress")
        .install_batch(runtime::Tokio)
        .unwrap();
    #[cfg(feature = "telemetry")]
    let span = trace::Tracer::start(&tracer, "root");
    #[cfg(feature = "telemetry")]
    let cx = <Context as trace::TraceContextExt>::current_with_span(span);

    let future = async move {
        let mut task = (0..config.total_client_count)
            .into_iter()
            .map(|i| create_task(config.clone(), i))
            .collect::<Vec<_>>();

        while !task.is_empty() {
            let mut new_task = vec![];
            for i in task {
                if let Err(id) = i.await.unwrap() {
                    new_task.push(create_task(config.clone(), id + 1000))
                }
            }
            task = new_task;
        }
    };

    #[cfg(feature = "telemetry")]
    let future = future.with_context(cx);

    future.await;
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let config = std::sync::Arc::new(StressConfig {
        server_ip: "127.0.0.1".to_string(),
        port_relay: 10025,
        port_submission: 10587,
        port_submissions: 10465,
        total_client_count: 100,
        mail_per_client: 10,
    });

    run_stress(config).await;

    #[cfg(feature = "telemetry")]
    global::shutdown_tracer_provider();

    Ok(())
}
