use vsmtp_common::{
    auth::Mechanism,
    mail_context::AuthCredentials,
    re::rsasl,
    state::StateSMTP,
    status::{InfoPacket, Status},
};
use vsmtp_config::Config;
use vsmtp_rule_engine::rule_engine::{RuleEngine, RuleState};

/// Backend of SASL implementation
pub type Backend = rsasl::DiscardOnDrop<
    rsasl::SASL<std::sync::Arc<Config>, std::sync::Arc<std::sync::RwLock<RuleEngine>>>,
>;

/// Function called by the SASL backend
pub struct Callback;

impl rsasl::Callback<std::sync::Arc<Config>, std::sync::Arc<std::sync::RwLock<RuleEngine>>>
    for Callback
{
    fn callback(
        sasl: &mut rsasl::SASL<
            std::sync::Arc<Config>,
            std::sync::Arc<std::sync::RwLock<RuleEngine>>,
        >,
        session: &mut vsmtp_common::re::rsasl::Session<
            std::sync::Arc<std::sync::RwLock<RuleEngine>>,
        >,
        prop: rsasl::Property,
    ) -> Result<(), rsasl::ReturnCode> {
        let config = unsafe { sasl.retrieve() }.ok_or(rsasl::ReturnCode::GSASL_INTEGRITY_ERROR)?;
        sasl.store(config.clone());

        let credentials = match prop {
            rsasl::Property::GSASL_PASSWORD => AuthCredentials::Query {
                authid: session
                    .get_property(rsasl::Property::GSASL_AUTHID)
                    .ok_or(rsasl::ReturnCode::GSASL_NO_AUTHID)?
                    .to_str()
                    .unwrap()
                    .to_string(),
            },
            rsasl::Property::GSASL_VALIDATE_SIMPLE => AuthCredentials::Verify {
                authid: session
                    .get_property(rsasl::Property::GSASL_AUTHID)
                    .ok_or(rsasl::ReturnCode::GSASL_NO_AUTHID)?
                    .to_str()
                    .unwrap()
                    .to_string(),
                authpass: session
                    .get_property(rsasl::Property::GSASL_PASSWORD)
                    .ok_or(rsasl::ReturnCode::GSASL_NO_PASSWORD)?
                    .to_str()
                    .unwrap()
                    .to_string(),
            },
            _ => return Err(rsasl::ReturnCode::GSASL_NO_CALLBACK),
        };

        let mut rule_state = RuleState::new(&config);
        {
            let guard = rule_state.get_context();

            guard
                .write()
                .map_err(|_| rsasl::ReturnCode::GSASL_INTEGRITY_ERROR)?
                .connection
                .credentials = Some(credentials);
        }

        let rule_engine = session
            .retrieve_mut()
            .ok_or(rsasl::ReturnCode::GSASL_INTEGRITY_ERROR)?;

        let result = rule_engine
            .read()
            .map_err(|_| rsasl::ReturnCode::GSASL_INTEGRITY_ERROR)?
            .run_when(
                &mut rule_state,
                &StateSMTP::Authentication(Mechanism::default(), None),
            );

        match prop {
            rsasl::Property::GSASL_VALIDATE_SIMPLE if result == Status::Accept => Ok(()),
            rsasl::Property::GSASL_PASSWORD => {
                let authpass = match result {
                    Status::Info(InfoPacket::Str(authpass)) => authpass,
                    _ => return Err(rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR),
                };

                session.set_property(rsasl::Property::GSASL_PASSWORD, authpass.as_bytes());
                Ok(())
            }
            _ => Err(rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR),
        }
    }
}
