#[ignore]
#[tokio::test]
async fn auth() {
    let client =
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous("localhost")
            .authentication(vec![
                lettre::transport::smtp::authentication::Mechanism::Plain,
                lettre::transport::smtp::authentication::Mechanism::Login,
                lettre::transport::smtp::authentication::Mechanism::Xoauth2,
            ])
            .credentials(lettre::transport::smtp::authentication::Credentials::from(
                ("hello", "world"),
            ))
            .port(10015)
            .build::<lettre::Tokio1Executor>();

    lettre::AsyncTransport::send(
        &client,
        lettre::Message::builder()
            .from("NoBody <nobody@domain.tld>".parse().unwrap())
            .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
            .to("Hei <hei@domain.tld>".parse().unwrap())
            .subject("Happy new year")
            .body(String::from("Be happy!"))
            .unwrap(),
    )
    .await
    .unwrap();
}
