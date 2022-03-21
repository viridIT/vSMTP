use vsmtp_common::{code::SMTPReplyCode, re::strum};

use crate::{
    config::{
        ConfigApp, ConfigAppLogs, ConfigAppVSL, ConfigServer, ConfigServerInterfaces,
        ConfigServerLogs, ConfigServerQueues, ConfigServerSMTP, ConfigServerSMTPError,
        ConfigServerSMTPTimeoutClient, ConfigServerSystem, ConfigServerSystemThreadPool,
    },
    Config,
};

use super::{wants::WantsValidate, with::Builder};

impl Builder<WantsValidate> {
    ///
    ///
    /// # Errors
    ///
    /// *
    pub fn validate(self) -> anyhow::Result<Config> {
        let app_services = self.state;
        let app_logs = app_services.parent;
        let app_vsl = app_logs.parent;
        let app = app_vsl.parent;
        let smtp_codes = app.parent;
        let smtp_error = smtp_codes.parent;
        let smtp_opt = smtp_error.parent;
        let srv_tls = smtp_opt.parent;
        let srv_delivery = srv_tls.parent;
        let srv_logs = srv_delivery.parent;
        let srv_inet = srv_logs.parent;
        let srv_syst = srv_inet.parent;
        let srv = srv_syst.parent;
        let version = srv.parent;

        Self::ensure(Config {
            version_requirement: version.version_requirement,
            server: ConfigServer {
                domain: srv.domain,
                client_count_max: srv.client_count_max,
                system: ConfigServerSystem {
                    user: srv_syst.user,
                    group: srv_syst.group,
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
                },
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
                },
                services: app_services.services,
            },
        })
    }

    pub(crate) fn ensure(mut config: Config) -> anyhow::Result<Config> {
        anyhow::ensure!(
            config.app.logs.filepath != config.server.logs.filepath,
            "rules and application logs cannot both be written in '{}' !",
            config.app.logs.filepath.display()
        );

        users::get_user_by_name(&config.server.system.user)
            .ok_or_else(|| anyhow::anyhow!("user not found: '{}'", config.server.system.user))?;
        users::get_group_by_name(&config.server.system.group)
            .ok_or_else(|| anyhow::anyhow!("group not found: '{}'", config.server.system.group))?;

        {
            let default_values = ConfigServerSMTP::default_smtp_codes();
            let reply_codes = &mut config.server.smtp.codes;
            for i in <SMTPReplyCode as strum::IntoEnumIterator>::iter() {
                reply_codes.insert(
                    i,
                    reply_codes
                        .get(&i)
                        .or_else(|| default_values.get(&i))
                        .unwrap()
                        .replace("{domain}", &config.server.domain),
                );
            }
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
            .with_default_app()
            .with_default_vsl_settings()
            .with_default_app_logs()
            .without_services()
            .validate();
        assert!(config.is_ok(), "{:?}", config);
    }
}
