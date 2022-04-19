/*
use lettre::transport::smtp::client::TlsParametersBuilder;
use vsmtp_server::re::tokio;

#[ignore = "require a server to run on port 10015"]
#[tokio::test]
async fn auth() {
    let client =
        lettre::AsyncSmtpTransport::<lettre::Tokio1Executor>::builder_dangerous("localhost")
            .tls(lettre::transport::smtp::client::Tls::Required(
                TlsParametersBuilder::new("localhost".to_string())
                    .dangerous_accept_invalid_certs(true)
                    .build()
                    .unwrap(),
            ))
            .authentication(vec![
                lettre::transport::smtp::authentication::Mechanism::Plain,
                lettre::transport::smtp::authentication::Mechanism::Login,
                lettre::transport::smtp::authentication::Mechanism::Xoauth2,
            ])
            .credentials(lettre::transport::smtp::authentication::Credentials::from(
                ("hél=lo", "wÖrld"),
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
*/
