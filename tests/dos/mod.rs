use std::thread;

use vsmtp::{config::server_config::ServerConfig, test_helpers::DefaultResolverTest};

const SERVER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const CLIENT_THREAD_COUNT: u64 = 1000;
const MAIL_PER_THREAD: u64 = 1000;

#[ignore = "too heavy"]
#[tokio::test]
async fn test_dos() {
    match fork::fork().expect("failed to fork process") {
        fork::Fork::Parent(_) => {
            let config: ServerConfig = toml::from_str(include_str!("dos.config.toml"))
                .expect("cannot parse config from toml");

            let server = config.build().await;

            log::warn!("Listening on: {:?}", server.addr());
            match tokio::time::timeout(
                SERVER_TIMEOUT,
                server.listen_and_serve(std::sync::Arc::new(tokio::sync::Mutex::new(
                    DefaultResolverTest {},
                ))),
            )
            .await
            {
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
