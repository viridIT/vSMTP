use crate::next::{
    config::{
        ConfigApp, ConfigAppLogs, ConfigAppVSL, ConfigServer, ConfigServerInterfaces,
        ConfigServerLogs, ConfigServerQueues, ConfigServerSMTP, ConfigServerSMTPError,
        ConfigServerSMTPTimeoutClient, ConfigServerSystem, ConfigServerSystemThreadPool,
        ConfigServerTls,
    },
    Config,
};

use super::{wants::WantsValidate, with::Builder};

impl Builder<WantsValidate> {
    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn validate(self) -> Config {
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
                tls: ConfigServerTls {
                    security_level: srv_tls.security_level,
                    preempt_cipherlist: srv_tls.preempt_cipherlist,
                    handshake_timeout: srv_tls.handshake_timeout,
                    protocol_version: srv_tls.protocol_version,
                    certificate: srv_tls.certificate,
                    private_key: srv_tls.private_key,
                    sni: srv_tls.sni,
                },
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

    pub(crate) const fn ensure(config: Config) -> Config {
        // TODO:
        config
    }
}

#[cfg(test)]
mod tests {
    use crate::next::Config;

    #[test]
    fn build_simple() {
        let _config = Config::builder()
            .with_current_version()
            .with_debug_server_info()
            .with_default_system()
            .with_ipv4_localhost_rfc()
            .with_default_log_settings()
            .with_default_queues()
            .with_safe_tls_config()
            .with_default_smtp_options()
            .with_default_smtp_error_handler()
            .with_default_smtp_codes()
            .with_default_app()
            .with_default_vsl_settings()
            .with_default_app_logs()
            .without_services()
            .validate();
    }
}
