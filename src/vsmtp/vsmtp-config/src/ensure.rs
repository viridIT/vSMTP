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
use crate::{config::field::FieldServerSMTP, Config};
use vsmtp_common::{
    auth::Mechanism,
    re::{anyhow, strum},
    CodeID, Reply, ReplyCode,
};

fn mech_list_to_code(list: &[Mechanism]) -> String {
    format!(
        "AUTH {}\r\n",
        list.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    )
}

impl Config {
    pub(crate) fn ensure(mut config: Self) -> anyhow::Result<Self> {
        anyhow::ensure!(
            config.app.logs.filepath != config.server.logs.filepath,
            "System and Application logs cannot both be written in '{}' !",
            config.app.logs.filepath.display()
        );

        anyhow::ensure!(
            config.server.system.thread_pool.processing != 0
                && config.server.system.thread_pool.receiver != 0
                && config.server.system.thread_pool.delivery != 0,
            "Worker threads cannot be set to 0"
        );

        {
            let auth_mechanism_list: Option<(Vec<Mechanism>, Vec<Mechanism>)> = config
                .server
                .smtp
                .auth
                .as_ref()
                .map(|auth| auth.mechanisms.iter().partition(|m| m.must_be_under_tls()));

            config.server.smtp.codes.insert(
                CodeID::EhloPain,
                Reply::new(
                    ReplyCode::Code { code: 250 },
                    [
                        &config.server.domain,
                        "\r\n",
                        &auth_mechanism_list
                            .as_ref()
                            .map(|(plain, secured)| {
                                if config
                                    .server
                                    .smtp
                                    .auth
                                    .as_ref()
                                    .map_or(false, |auth| auth.enable_dangerous_mechanism_in_clair)
                                {
                                    mech_list_to_code(&[secured.clone(), plain.clone()].concat())
                                } else {
                                    mech_list_to_code(secured)
                                }
                            })
                            .unwrap_or_default(),
                        "STARTTLS\r\n",
                        "8BITMIME\r\n",
                        "SMTPUTF8\r\n",
                    ]
                    .concat(),
                ),
            );

            config.server.smtp.codes.insert(
                CodeID::EhloSecured,
                Reply::new(
                    ReplyCode::Code { code: 250 },
                    [
                        &config.server.domain,
                        "\r\n",
                        &auth_mechanism_list
                            .as_ref()
                            .map(|(must_be_secured, _)| mech_list_to_code(must_be_secured))
                            .unwrap_or_default(),
                        "8BITMIME\r\n",
                        "SMTPUTF8\r\n",
                    ]
                    .concat(),
                ),
            );
        }

        let default_values = FieldServerSMTP::default_smtp_codes();
        let reply_codes = &mut config.server.smtp.codes;

        for key in <CodeID as strum::IntoEnumIterator>::iter() {
            reply_codes
                .entry(key)
                .or_insert_with_key(|key| default_values.get(key).expect("missing code").clone());

            reply_codes.entry(key).and_modify(|reply| {
                reply.set(reply.text().replace("{domain}", &config.server.domain));
            });
        }

        Ok(config)
    }
}
