/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
use vsmtp_common::{
    auth::Credentials, auth::Mechanism, mail_context::ConnectionContext, re::vsmtp_rsasl,
    state::StateSMTP, status::Status,
};
use vsmtp_config::{Config, Resolvers};
use vsmtp_rule_engine::{RuleEngine, RuleState};

type SessionState = (
    std::sync::Arc<RuleEngine>,
    std::sync::Arc<Resolvers>,
    ConnectionContext,
);

/// Backend of SASL implementation
pub type Backend =
    vsmtp_rsasl::DiscardOnDrop<vsmtp_rsasl::SASL<std::sync::Arc<Config>, SessionState>>;

/// SASL session data.
pub type Session = vsmtp_rsasl::Session<SessionState>;

/// Function called by the SASL backend
pub struct Callback;

impl vsmtp_rsasl::Callback<std::sync::Arc<Config>, SessionState> for Callback {
    fn callback(
        sasl: &mut vsmtp_rsasl::SASL<std::sync::Arc<Config>, SessionState>,
        session: &mut Session,
        prop: vsmtp_rsasl::Property,
    ) -> Result<(), vsmtp_rsasl::ReturnCode> {
        #[allow(unsafe_code)]
        let config =
            unsafe { sasl.retrieve() }.ok_or(vsmtp_rsasl::ReturnCode::GSASL_INTEGRITY_ERROR)?;
        sasl.store(config.clone());

        let credentials = match prop {
            vsmtp_rsasl::Property::GSASL_PASSWORD => Credentials::Query {
                authid: session
                    .get_property(vsmtp_rsasl::Property::GSASL_AUTHID)
                    .ok_or(vsmtp_rsasl::ReturnCode::GSASL_NO_AUTHID)?
                    .to_str()
                    .unwrap()
                    .to_string(),
            },
            vsmtp_rsasl::Property::GSASL_VALIDATE_SIMPLE => Credentials::Verify {
                authid: session
                    .get_property(vsmtp_rsasl::Property::GSASL_AUTHID)
                    .ok_or(vsmtp_rsasl::ReturnCode::GSASL_NO_AUTHID)?
                    .to_str()
                    .unwrap()
                    .to_string(),
                authpass: session
                    .get_property(vsmtp_rsasl::Property::GSASL_PASSWORD)
                    .ok_or(vsmtp_rsasl::ReturnCode::GSASL_NO_PASSWORD)?
                    .to_str()
                    .unwrap()
                    .to_string(),
            },
            vsmtp_rsasl::Property::GSASL_VALIDATE_ANONYMOUS => Credentials::AnonymousToken {
                token: session
                    .get_property(vsmtp_rsasl::Property::GSASL_ANONYMOUS_TOKEN)
                    .ok_or(vsmtp_rsasl::ReturnCode::GSASL_NO_ANONYMOUS_TOKEN)?
                    .to_str()
                    .unwrap()
                    .to_string(),
            },
            _ => return Err(vsmtp_rsasl::ReturnCode::GSASL_NO_CALLBACK),
        };

        let (rule_engine, resolvers, conn) = session
            .retrieve_mut()
            .ok_or(vsmtp_rsasl::ReturnCode::GSASL_INTEGRITY_ERROR)?;

        let mut conn = conn.clone();
        conn.credentials = Some(credentials);

        let result = {
            let mut rule_state =
                RuleState::with_connection(&config, resolvers.clone(), rule_engine, conn);

            rule_engine.run_when(
                &mut rule_state,
                &StateSMTP::Authenticate(Mechanism::default(), None),
            )
        };

        match prop {
            vsmtp_rsasl::Property::GSASL_VALIDATE_SIMPLE
            | vsmtp_rsasl::Property::GSASL_VALIDATE_ANONYMOUS
                if matches!(result, Status::Accept(..)) =>
            {
                Ok(())
            }
            vsmtp_rsasl::Property::GSASL_PASSWORD => {
                let authpass = match result {
                    Status::Packet(authpass) => authpass,
                    _ => return Err(vsmtp_rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR),
                };

                session.set_property(vsmtp_rsasl::Property::GSASL_PASSWORD, authpass.as_bytes());
                Ok(())
            }
            _ => Err(vsmtp_rsasl::ReturnCode::GSASL_AUTHENTICATION_ERROR),
        }
    }
}
