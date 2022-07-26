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
    config::field::{
        FieldApp, FieldAppLogs, FieldAppVSL, FieldServer, FieldServerInterfaces, FieldServerLogs,
        FieldServerQueues, FieldServerSMTP, FieldServerSMTPError, FieldServerSMTPTimeoutClient,
        FieldServerSystem, FieldServerSystemThreadPool,
    },
    Config,
};
use vsmtp_common::re::anyhow;

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
            server: FieldServer {
                domain: srv.domain,
                client_count_max: srv.client_count_max,
                system: FieldServerSystem {
                    user: srv_syst.user,
                    group: srv_syst.group,
                    group_local: srv_syst.group_local,
                    thread_pool: FieldServerSystemThreadPool {
                        receiver: srv_syst.thread_pool_receiver,
                        processing: srv_syst.thread_pool_processing,
                        delivery: srv_syst.thread_pool_delivery,
                    },
                },
                interfaces: FieldServerInterfaces {
                    addr: srv_inet.addr,
                    addr_submission: srv_inet.addr_submission,
                    addr_submissions: srv_inet.addr_submissions,
                },
                logs: FieldServerLogs {
                    filepath: srv_logs.filepath,
                    format: srv_logs.format,
                    level: srv_logs.level,
                },
                queues: FieldServerQueues {
                    dirpath: srv_delivery.dirpath,
                    working: srv_delivery.working,
                    delivery: srv_delivery.delivery,
                },
                tls: srv_tls.tls,
                smtp: FieldServerSMTP {
                    rcpt_count_max: smtp_opt.rcpt_count_max,
                    disable_ehlo: smtp_opt.disable_ehlo,
                    required_extension: smtp_opt.required_extension,
                    error: FieldServerSMTPError {
                        soft_count: smtp_error.error.soft_count,
                        hard_count: smtp_error.error.hard_count,
                        delay: smtp_error.error.delay,
                    },
                    timeout_client: FieldServerSMTPTimeoutClient {
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
                dkim: None,
            },
            app: FieldApp {
                dirpath: app.dirpath,
                vsl: FieldAppVSL {
                    filepath: app_vsl.filepath,
                },
                logs: FieldAppLogs {
                    filepath: app_logs.filepath,
                    format: app_logs.format,
                },
            },
        })
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
