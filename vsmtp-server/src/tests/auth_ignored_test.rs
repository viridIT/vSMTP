#[tokio::test]
#[ignore]
async fn test() {
    let sender =
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous("127.0.0.1")
            .port(10025)
            .authentication(vec![
                lettre::transport::smtp::authentication::Mechanism::Plain,
            ])
            .credentials(lettre::transport::smtp::authentication::Credentials::new(
                option_env!("AUTHID").unwrap().to_string(),
                option_env!("AUTHPASS").unwrap().to_string(),
            ))
            .build::<lettre::Tokio1Executor>();

    let email = lettre::Message::builder()
        .from("NoBody <nobody@domain.tld>".parse().unwrap())
        .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
        .to("Hei <hei@domain.tld>".parse().unwrap())
        .subject("Happy new year")
        .body(String::from("Be happy!"))
        .unwrap();

    lettre::AsyncTransport::send(&sender, email).await.unwrap();
}
