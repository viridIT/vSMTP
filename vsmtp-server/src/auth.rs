use vsmtp_common::{auth::Mechanism, re::rsasl, state::StateSMTP, status::Status};
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
        // FIXME: this db MUST be provided by the rule engine
        // which authorize the credentials or lookup a database (sql/ldap/...)
        // or call an external services (saslauthd) for example
        let db = [("hello", "world"), ("héllo", "wÖrld")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<std::collections::HashMap<String, String>>();

        let config = unsafe { sasl.retrieve() }.unwrap();
        sasl.store(config.clone());

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

                let mut rule_state = RuleState::new(&config);
                {
                    let guard = rule_state.get_context();

                    let mut ctx = guard.write().unwrap();
                    ctx.connection.authid = authid;
                    ctx.connection.authpass = password;
                }

                let rule_engine = session.retrieve_mut().unwrap();

                let result = {
                    rule_engine.read().unwrap().run_when(
                        &mut rule_state,
                        &StateSMTP::Authentication(Mechanism::default(), None),
                    )
                };

                if result == Status::Accept {
                    Ok(())
                } else {
                    Err(rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR)
                }
            }
            _ => Err(rsasl::ReturnCode::GSASL_NO_CALLBACK),
        }
    }
}
