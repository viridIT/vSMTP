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
use super::{wants::WantsValidate, with::Builder};
use crate::{
    config::{
        ConfigApp, ConfigAppLogs, ConfigAppVSL, ConfigServer, ConfigServerInterfaces,
        ConfigServerLogs, ConfigServerQueues, ConfigServerSMTP, ConfigServerSMTPError,
        ConfigServerSMTPTimeoutClient, ConfigServerSystem, ConfigServerSystemThreadPool,
    },
    Config,
};
use vsmtp_common::{
    auth::Mechanism,
    re::{anyhow, strum},
    CodeID, Reply, ReplyCode,
};

impl Builder<WantsValidate> {
    ///
    ///
    /// # Errors
    ///
    /// *
    pub fn validate(self) -> anyhow::Result<Config> {
        let virtual_entries = self.state;
        let dns = virtual_entries.parent;
        let app_logs = dns.parent;
        let app_vsl = app_logs.parent;
        let app = app_vsl.parent;
        let auth = app.parent;
        let smtp_codes = auth.parent;
        let smtp_error = smtp_codes.parent;
        let smtp_opt = smtp_error.parent;
        let srv_tls = smtp_opt.parent;
        let srv_delivery = srv_tls.parent;
        let srv_logs = srv_delivery.parent;
        let srv_inet = srv_logs.parent;
        let srv_syst = srv_inet.parent;
        let srv = srv_syst.parent;
        let version = srv.parent;

        Config::ensure(Config {
            version_requirement: version.version_requirement,
            server: ConfigServer {
                domain: srv.domain,
                client_count_max: srv.client_count_max,
                system: ConfigServerSystem {
                    user: srv_syst.user,
                    group: srv_syst.group,
                    group_local: srv_syst.group_local,
                    thread_pool: ConfigServerSystemThreadPool {
                        receiver: srv_syst.thread_pool_receiver,
                        processing: srv_syst.thread_pool_processing,
                        delivery: srv_syst.thread_pool_delivery,
                    },
                },
                interfaces: ConfigServerInterfaces {
                    addr: srv_inet.addr,
                    addr_submission: srv_inet.addr_submission,
                    addr_submissions: srv_inet.addr_submissions,
                },
                logs: ConfigServerLogs {
                    filepath: srv_logs.filepath,
                    format: srv_logs.format,
                    level: srv_logs.level,
                    size_limit: srv_logs.size_limit,
                    archive_count: srv_logs.archive_count,
                },
                queues: ConfigServerQueues {
                    dirpath: srv_delivery.dirpath,
                    working: srv_delivery.working,
                    delivery: srv_delivery.delivery,
                },
                tls: srv_tls.tls,
                smtp: ConfigServerSMTP {
                    rcpt_count_max: smtp_opt.rcpt_count_max,
                    disable_ehlo: smtp_opt.disable_ehlo,
                    required_extension: smtp_opt.required_extension,
                    error: ConfigServerSMTPError {
                        soft_count: smtp_error.error.soft_count,
                        hard_count: smtp_error.error.hard_count,
                        delay: smtp_error.error.delay,
                    },
                    timeout_client: ConfigServerSMTPTimeoutClient {
                        connect: smtp_error.timeout_client.connect,
                        helo: smtp_error.timeout_client.helo,
                        mail_from: smtp_error.timeout_client.mail_from,
                        rcpt_to: smtp_error.timeout_client.rcpt_to,
                        data: smtp_error.timeout_client.data,
                    },
                    codes: smtp_codes.codes,
                    auth: auth.auth,
                },
                dns: dns.config,
                r#virtual: virtual_entries.r#virtual,
            },
            app: ConfigApp {
                dirpath: app.dirpath,
                vsl: ConfigAppVSL {
                    filepath: app_vsl.filepath,
                },
                logs: ConfigAppLogs {
                    filepath: app_logs.filepath,
                    level: app_logs.level,
                    format: app_logs.format,
                    size_limit: app_logs.size_limit,
                    archive_count: app_logs.archive_count,
                },
            },
        })
    }
}

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

        let default_values = ConfigServerSMTP::default_smtp_codes();
        let reply_codes = &mut config.server.smtp.codes;

        for key in <CodeID as strum::IntoEnumIterator>::iter() {
            reply_codes
                .entry(key)
                .or_insert_with_key(|key| default_values.get(key).unwrap().clone());

            reply_codes.entry(key).and_modify(|reply| {
                reply.set(reply.text().replace("{domain}", &config.server.domain));
            });
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use crate::Config;

    #[test]
    fn default_build() {
        let config = Config::builder()
            .with_current_version()
            .with_debug_server_info()
            .with_default_system()
            .with_ipv4_localhost()
            .with_default_logs_settings()
            .with_default_delivery()
            .without_tls_support()
            .with_default_smtp_options()
            .with_default_smtp_error_handler()
            .with_default_smtp_codes()
            .without_auth()
            .with_default_app()
            .with_default_vsl_settings()
            .with_default_app_logs()
            .with_system_dns()
            .without_virtual_entries()
            .validate();
        assert!(config.is_ok(), "{:?}", config);
    }
}
