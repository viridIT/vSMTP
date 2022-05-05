use lettre::transport::smtp::{
    authentication::{Credentials, Mechanism},
    client::{Tls, TlsParameters},
};
use opentelemetry::{global, runtime, trace, Context};

async fn start_server() -> std::io::Result<()> {
    let vsmtp_path = std::path::PathBuf::from_iter(["./target/release/vsmtp"]);
    let config_path = std::path::PathBuf::from_iter(["./benchmarks/stress/vsmtp.stress.toml"]);

    let output = std::process::Command::new(vsmtp_path)
        .args([
            "-t",
            "10s",
            "--no-daemon",
            "-c",
            config_path.to_str().unwrap(),
        ])
        .spawn()?
        .wait_with_output()?;

    println!("{:?}", output);

    Ok(())
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
    let tracer = global::tracer("client");
    let span = trace::Tracer::start(&tracer, format!("Connection: {client_nb}"));
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

    for i in 0..config.mail_per_client {
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

fn create_task(
    config: std::sync::Arc<StressConfig>,
    id: u64,
) -> tokio::task::JoinHandle<std::result::Result<(), u64>> {
    let tracer = global::tracer("register-task");
    let span = trace::Tracer::start(&tracer, format!("Register Task: {id}"));
    let cx = <Context as trace::TraceContextExt>::current_with_span(span);

    tokio::spawn(trace::FutureExt::with_context(
        run_one_connection(config, id),
        cx,
    ))
}

async fn run_stress(config: std::sync::Arc<StressConfig>) {
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("vsmtp-stress")
        .install_batch(runtime::Tokio)
        .unwrap();

    let span = trace::Tracer::start(&tracer, "root");
    let cx = <Context as trace::TraceContextExt>::current_with_span(span);

    trace::FutureExt::with_context(
        async move {
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
        },
        cx,
    )
    .await;
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let server = tokio::spawn(start_server());

    let config = std::sync::Arc::new(StressConfig {
        server_ip: "127.0.0.1".to_string(),
        port_relay: 10025,
        port_submission: 10587,
        port_submissions: 10465,
        total_client_count: 1000,
        mail_per_client: 1,
    });

    let clients = run_stress(config);

    tokio::select! {
        s = server => s??,
        c = clients => c
    };

    global::shutdown_tracer_provider();

    Ok(())
}
