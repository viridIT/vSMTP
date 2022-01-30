const MTA_PORT_PLAIN: u16 = 10025;

fn get_mail() -> lettre::Message {
    lettre::Message::builder()
        .from("NoBody <nobody@domain.tld>".parse().unwrap())
        .reply_to("Yuin <yuin@domain.tld>".parse().unwrap())
        .to("Hei <hei@domain.tld>".parse().unwrap())
        .subject("Happy new year")
        .body(String::from("Be happy!"))
        .unwrap()
}

#[test]
#[ignore = "require a server running"]
fn send_mail_plain() {
    let email = get_mail();

    let mailer = lettre::SmtpTransport::builder_dangerous("localhost")
        .port(MTA_PORT_PLAIN)
        .build();

    match lettre::Transport::send(&mailer, &email) {
        Ok(_) => println!("Email sent successfully!"),
        Err(e) => panic!("Could not send email: {:?}", e),
    }
}

#[test]
#[ignore = "require a server running and supporting tls"]
fn send_mail_starttls() {
    let email = get_mail();

    let mailer = lettre::SmtpTransport::builder_dangerous("localhost")
        .tls(lettre::transport::smtp::client::Tls::Required(
            lettre::transport::smtp::client::TlsParameters::builder("example.com".to_string())
                .dangerous_accept_invalid_certs(true)
                .build()
                .unwrap(),
        ))
        .port(MTA_PORT_PLAIN)
        .build();

    match lettre::Transport::send(&mailer, &email) {
        Ok(_) => println!("Email sent successfully!"),
        Err(e) => panic!("Could not send email: {:?}", e),
    }
}

// FIXME: support fully tunneled tls connection
/*
#[test]
#[ignore = "require a server running and supporting tunneled tls"]
fn send_mail_tunneled_tls() {
    let email = get_mail();

    let mailer = lettre::SmtpTransport::builder_dangerous("localhost")
        .tls(lettre::transport::smtp::client::Tls::Wrapper(
            lettre::transport::smtp::client::TlsParameters::builder("example.com".to_string())
                .dangerous_accept_invalid_certs(true)
                .build()
                .unwrap(),
        ))
        .port(MTA_PORT_PLAIN)
        .build();

    match lettre::Transport::send(&mailer, &email) {
        Ok(_) => println!("Email sent successfully!"),
        Err(e) => panic!("Could not send email: {:?}", e),
    }
}
*/
