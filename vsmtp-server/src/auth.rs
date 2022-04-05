use vsmtp_common::re::rsasl;

/// Backend of SASL implementation
pub type Backend = rsasl::DiscardOnDrop<rsasl::SASL<(), ()>>;

/// Function called by the SASL backend
pub struct Callback;

impl rsasl::Callback<(), ()> for Callback {
    fn callback(
        _sasl: &mut rsasl::SASL<(), ()>,
        session: &mut rsasl::Session<()>,
        prop: rsasl::Property,
    ) -> Result<(), rsasl::ReturnCode> {
        // FIXME: this db MUST be provided by the rule engine
        // which authorize the credentials or lookup a database (sql/ldap/...)
        // or call an external services (saslauthd) for example
        let db = [("hello", "world"), ("héllo", "wÖrld")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<std::collections::HashMap<String, String>>();

        match prop {
            rsasl::Property::GSASL_PASSWORD => {
                let authid = session
                    .get_property(rsasl::Property::GSASL_AUTHID)
                    .ok_or(rsasl::ReturnCode::GSASL_NO_AUTHID)?
                    .to_str()
                    .unwrap()
                    .to_string();

                if let Some(pass) = db.get(&authid) {
                    session.set_property(rsasl::Property::GSASL_PASSWORD, pass.as_bytes());
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
