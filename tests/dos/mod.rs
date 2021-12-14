use vsmtp::{
    config::server_config::ServerConfig, mailprocessing::mail_receiver::StateSMTP,
    model::mail::MailContext, smtp::code::SMTPReplyCode,
};

const SERVER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
const CLIENT_COUNT: u64 = 1_000_000;

#[ignore = "too heavy"]
#[tokio::test]
async fn test_dos() {
    struct R;

    #[async_trait::async_trait]
    impl vsmtp::resolver::DataEndResolver for R {
        async fn on_data_end(
            _: &ServerConfig,
            _: usize,
            _: &MailContext,
        ) -> (StateSMTP, SMTPReplyCode) {
            (StateSMTP::MailFrom, SMTPReplyCode::Code250)
        }
    }

    match fork::fork().expect("failed to fork process") {
        fork::Fork::Parent(_) => {
            let config: ServerConfig = toml::from_str(include_str!("dos.config.toml"))
                .expect("cannot parse config from toml");

            let server = config.build::<R>().await;

            log::warn!("Listening on: {:?}", server.addr());
            match tokio::time::timeout(SERVER_TIMEOUT, server.listen_and_serve()).await {
                Ok(Ok(_)) => todo!(),
                Ok(Err(e)) => panic!("{}", e),
                Err(t) => panic!("{}", t),
            };
        }
        fork::Fork::Child => {
            let mailer = lettre::SmtpTransport::builder_dangerous("0.0.0.0")
                .port(10027)
                .build();

            let mut rng = rand::thread_rng();

            for i in 0..CLIENT_COUNT {
                let email = lettre::Message::builder()
                    .from("NoBody <nobody@domain.tld>".parse().unwrap())
                    .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
                    .to("Hei <hei@domain.tld>".parse().unwrap())
                    .subject(format!("DOS {}", i))
                    .body(
                        (0..rand::Rng::gen::<u16>(&mut rng))
                            .map(|_| rand::Rng::gen::<u8>(&mut rng))
                            .collect::<Vec<_>>(),
                    )
                    .unwrap();

                match lettre::Transport::send(&mailer, &email) {
                    Ok(_) => {}
                    Err(e) => {
                        let mut file = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open("./tests/generated/failed.log")
                            .unwrap();

                        std::io::Write::write_fmt(&mut file, format_args!("{}\n", i)).unwrap();
                        log::warn!("Could not send email: {:?}", e);
                    }
                }
            }
        }
    };
}
