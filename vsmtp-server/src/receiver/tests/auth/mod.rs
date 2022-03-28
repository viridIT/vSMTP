use vsmtp_common::{code::SMTPReplyCode, re::rsasl};
use vsmtp_config::Config;

use crate::receiver::test_helpers::get_regular_config;

fn get_auth_config() -> Config {
    // TODO: make selection of SMTP extension and AUTH mechanism more simple

    let mut config = get_regular_config();
    config.server.smtp.codes.insert(
        SMTPReplyCode::Code250PlainEsmtp,
        [
            "250-testserver.com\r\n",
            "250-8BITMIME\r\n",
            "250-SMTPUTF8\r\n",
            "250-AUTH PLAIN\r\n",
            "250 STARTTLS\r\n",
        ]
        .concat(),
    );
    config
}

struct TestAuth;

impl rsasl::Callback<(), ()> for TestAuth {
    fn callback(
        _sasl: &mut rsasl::SASL<(), ()>,
        session: &mut rsasl::Session<()>,
        prop: rsasl::Property,
    ) -> Result<(), rsasl::ReturnCode> {
        match prop {
            rsasl::Property::GSASL_PASSWORD => {
                let authid = session
                    .get_property(rsasl::Property::GSASL_AUTHID)
                    .ok_or(rsasl::ReturnCode::GSASL_NO_AUTHID)?
                    .to_str()
                    .unwrap()
                    .to_string();

                // println!("{}", authid);
                if authid == "hello" {
                    session.set_property(rsasl::Property::GSASL_PASSWORD, b"world");
                }

                Ok(())
            }
            rsasl::Property::GSASL_VALIDATE_SIMPLE => {
                let (authid, password) = (
                    session
                        .get_property(rsasl::Property::GSASL_AUTHID)
                        .ok_or(rsasl::ReturnCode::GSASL_NO_AUTHID)?
                        .to_str()
                        .unwrap()
                        .to_string(),
                    session
                        .get_property(rsasl::Property::GSASL_PASSWORD)
                        .ok_or(rsasl::ReturnCode::GSASL_NO_PASSWORD)?
                        .to_str()
                        .unwrap()
                        .to_string(),
                );

                let db = [("hello", "world"), ("héllo", "wÖrld")]
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect::<std::collections::HashMap<String, String>>();

                if db.get(&authid).map_or(false, |p| *p == password) {
                    Ok(())
                } else {
                    Err(rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR)
                }
            }
            _ => Err(rsasl::ReturnCode::GSASL_NO_CALLBACK),
        }
    }
}

mod all_mechanism;
mod basic;
mod send_to_server;
