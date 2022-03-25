use vsmtp_common::re::rsasl;

#[allow(clippy::module_name_repetitions)]
pub struct AuthCallback;

impl rsasl::Callback<(), ()> for AuthCallback {
    fn callback(
        _sasl: &mut rsasl::SASL<(), ()>,
        session: &mut rsasl::Session<()>,
        prop: rsasl::Property,
    ) -> Result<(), rsasl::ReturnCode> {
        match prop {
            rsasl::Property::GSASL_VALIDATE_SIMPLE => {
                // Access the authentication id, i.e. the username to check the password for
                let authcid = session
                    .get_property(rsasl::Property::GSASL_AUTHID)
                    .ok_or(rsasl::ReturnCode::GSASL_NO_AUTHID)?
                    .to_str()
                    .unwrap()
                    .to_string();

                // Access the password itself
                let password = session
                    .get_property(rsasl::Property::GSASL_PASSWORD)
                    .ok_or(rsasl::ReturnCode::GSASL_NO_PASSWORD)?
                    .to_str()
                    .unwrap()
                    .to_string();

                // For brevity sake we use hard-coded credentials here.
                if authcid == "hél=lo" && password == "wÖrld" {
                    Ok(())
                } else {
                    Err(rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR)
                }
            }
            _ => Err(rsasl::ReturnCode::GSASL_NO_CALLBACK),
        }
    }
}
