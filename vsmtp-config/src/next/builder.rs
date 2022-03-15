use vsmtp_common::code::SMTPReplyCode;

use crate::TlsSecurityLevel;

use super::{
    config::{
        ConfigApp, ConfigAppLogs, ConfigAppVSL, ConfigQueueDelivery, ConfigQueueWorking,
        ConfigServer, ConfigServerInterfaces, ConfigServerLogs, ConfigServerQueues,
        ConfigServerSMTP, ConfigServerSMTPError, ConfigServerSMTPTimeoutClient, ConfigServerSystem,
        ConfigServerSystemThreadPool, ConfigServerTls, ConfigServerTlsSni, Service,
    },
    Config,
};

///
pub struct Builder<State> {
    pub(crate) state: State,
}

///
pub struct WantsVersion(pub(crate) ());

///
pub struct WantsServer {
    #[allow(dead_code)]
    pub(crate) parent: WantsVersion,
    version_requirement: semver::VersionReq,
}

pub struct WantsServerSystem {
    pub(crate) parent: WantsServer,
    domain: String,
    client_count_max: u32,
}

pub struct WantsServerInterfaces {
    pub(crate) parent: WantsServerSystem,
    user: String,
    group: String,
    thread_pool_receiver: u32,
    thread_pool_processing: u32,
    thread_pool_delivery: u32,
}

pub struct WantsServerLogs {
    pub(crate) parent: WantsServerInterfaces,
    addr: Vec<std::net::SocketAddr>,
    addr_submission: Vec<std::net::SocketAddr>,
    addr_submissions: Vec<std::net::SocketAddr>,
}

pub struct WantsServerQueues {
    pub(crate) parent: WantsServerLogs,
    filepath: std::path::PathBuf,
    format: String,
    level: std::collections::BTreeMap<String, log::LevelFilter>,
}

pub struct WantsServerTLSConfig {
    pub(crate) parent: WantsServerQueues,
    dirpath: std::path::PathBuf,
    working: ConfigQueueWorking,
    delivery: ConfigQueueDelivery,
}

pub struct WantsServerSMTPConfig1 {
    pub(crate) parent: WantsServerTLSConfig,
    security_level: TlsSecurityLevel,
    preempt_cipherlist: bool,
    handshake_timeout: std::time::Duration,
    protocol_version: Vec<rustls::ProtocolVersion>,
    certificate: rustls::Certificate,
    private_key: rustls::PrivateKey,
    sni: Vec<ConfigServerTlsSni>,
}

pub struct WantsServerSMTPConfig2 {
    pub(crate) parent: WantsServerSMTPConfig1,
    rcpt_count_max: u32,
    disable_ehlo: bool,
    required_extension: Vec<String>,
}

pub struct WantsServerSMTPConfig3 {
    pub(crate) parent: WantsServerSMTPConfig2,
    error: ConfigServerSMTPError,
    timeout_client: ConfigServerSMTPTimeoutClient,
}

pub struct WantsApp {
    pub(crate) parent: WantsServerSMTPConfig3,
    codes: std::collections::BTreeMap<SMTPReplyCode, String>,
}

pub struct WantsAppVSL {
    pub(crate) parent: WantsApp,
    dirpath: std::path::PathBuf,
}

pub struct WantsAppLogs {
    pub(crate) parent: WantsAppVSL,
    filepath: std::path::PathBuf,
}

pub struct WantsAppServices {
    pub(crate) parent: WantsAppLogs,
    filepath: std::path::PathBuf,
    level: log::LevelFilter,
    format: String,
}

pub struct WantsValidate {
    pub(crate) parent: WantsAppServices,
    services: std::collections::BTreeMap<String, Service>,
}

impl Builder<WantsVersion> {
    ///
    ///
    /// # Panics
    ///
    /// * CARGO_PKG_VERSION is not valid
    #[must_use]
    pub fn with_current_version(self) -> Builder<WantsServer> {
        Builder::<WantsServer> {
            state: WantsServer {
                parent: self.state,
                version_requirement: semver::VersionReq::parse(env!("CARGO_PKG_VERSION")).unwrap(),
            },
        }
    }
}

impl Builder<WantsServer> {
    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_debug_server_info(self) -> Builder<WantsServerSystem> {
        Builder::<WantsServerSystem> {
            state: WantsServerSystem {
                parent: self.state,
                domain: "debug.com".to_string(),
                client_count_max: 32,
            },
        }
    }
}

impl Builder<WantsServerSystem> {
    ///
    #[must_use]
    pub fn with_default_system(self) -> Builder<WantsServerInterfaces> {
        Builder::<WantsServerInterfaces> {
            state: WantsServerInterfaces {
                parent: self.state,
                user: "vsmtp".to_string(),
                group: "vsmtp".to_string(),
                thread_pool_receiver: 6,
                thread_pool_processing: 6,
                thread_pool_delivery: 6,
            },
        }
    }
}

impl Builder<WantsServerInterfaces> {
    ///
    #[must_use]
    pub fn with_ipv4_localhost_rfc(self) -> Builder<WantsServerLogs> {
        Builder::<WantsServerLogs> {
            state: WantsServerLogs {
                parent: self.state,
                addr: vec!["0.0.0.0:25".parse().expect("valid")],
                addr_submission: vec!["0.0.0.0:587".parse().expect("valid")],
                addr_submissions: vec!["0.0.0.0:465".parse().expect("valid")],
            },
        }
    }
}

impl Builder<WantsServerLogs> {
    ///
    #[must_use]
    pub fn with_default_log_settings(self) -> Builder<WantsServerQueues> {
        Builder::<WantsServerQueues> {
            state: WantsServerQueues {
                parent: self.state,
                filepath: "/var/log/vsmtp/vsmtp.log".into(),
                format: "{d} {l} - ".into(),
                level: std::collections::BTreeMap::new(),
            },
        }
    }
}

impl Builder<WantsServerQueues> {
    ///
    #[must_use]
    pub fn with_default_queues(self) -> Builder<WantsServerTLSConfig> {
        Builder::<WantsServerTLSConfig> {
            state: WantsServerTLSConfig {
                parent: self.state,
                dirpath: "/var/log/vsmtp/vsmtp.log".into(),
                working: ConfigQueueWorking { channel_size: 32 },
                delivery: ConfigQueueDelivery {
                    channel_size: 32,
                    deferred_retry_max: 100,
                    deferred_retry_period: std::time::Duration::from_secs(30),
                },
            },
        }
    }
}

impl Builder<WantsServerTLSConfig> {
    ///
    #[must_use]
    pub fn with_safe_tls_config(self) -> Builder<WantsServerSMTPConfig1> {
        Builder::<WantsServerSMTPConfig1> {
            state: WantsServerSMTPConfig1 {
                parent: self.state,
                security_level: TlsSecurityLevel::May,
                preempt_cipherlist: false,
                handshake_timeout: std::time::Duration::from_millis(200),
                protocol_version: vec![rustls::ProtocolVersion::TLSv1_3],
                certificate: rustls::Certificate(vec![]),
                private_key: rustls::PrivateKey(vec![]),
                sni: vec![],
            },
        }
    }
}

impl Builder<WantsServerSMTPConfig1> {
    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_default_smtp_options(self) -> Builder<WantsServerSMTPConfig2> {
        Builder::<WantsServerSMTPConfig2> {
            state: WantsServerSMTPConfig2 {
                parent: self.state,
                rcpt_count_max: 32,
                disable_ehlo: false,
                required_extension: vec![],
            },
        }
    }
}

impl Builder<WantsServerSMTPConfig2> {
    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_default_smtp_error_handler(self) -> Builder<WantsServerSMTPConfig3> {
        Builder::<WantsServerSMTPConfig3> {
            state: WantsServerSMTPConfig3 {
                parent: self.state,
                error: ConfigServerSMTPError {
                    soft_count: 5,
                    hard_count: 10,
                    delay: std::time::Duration::from_secs(2000),
                },
                timeout_client: ConfigServerSMTPTimeoutClient {
                    connect: std::time::Duration::from_secs(1),
                    helo: std::time::Duration::from_secs(1),
                    mail_from: std::time::Duration::from_secs(1),
                    rcpt_to: std::time::Duration::from_secs(1),
                    data: std::time::Duration::from_secs(1),
                },
            },
        }
    }
}

impl Builder<WantsServerSMTPConfig3> {
    ///
    #[must_use]
    pub fn with_default_smtp_codes(self) -> Builder<WantsApp> {
        Builder::<WantsApp> {
            state: WantsApp {
                parent: self.state,
                codes: std::collections::BTreeMap::new(),
            },
        }
    }
}

impl Builder<WantsApp> {
    ///
    #[must_use]
    pub fn with_default_app(self) -> Builder<WantsAppVSL> {
        Builder::<WantsAppVSL> {
            state: WantsAppVSL {
                parent: self.state,
                dirpath: "/var/spool/vsmtp/app".into(),
            },
        }
    }
}

impl Builder<WantsAppVSL> {
    ///
    #[must_use]
    pub fn with_default_vsl_settings(self) -> Builder<WantsAppLogs> {
        Builder::<WantsAppLogs> {
            state: WantsAppLogs {
                parent: self.state,
                filepath: "/etc/vsmtp/main.vsl".into(),
            },
        }
    }
}

impl Builder<WantsAppLogs> {
    ///
    #[must_use]
    pub fn with_default_app_logs(self) -> Builder<WantsAppServices> {
        Builder::<WantsAppServices> {
            state: WantsAppServices {
                parent: self.state,
                filepath: "/var/log/vsmtp/app.log".into(),
                level: log::LevelFilter::Trace,
                format: "{d} - {m}{n}".to_string(),
            },
        }
    }
}

impl Builder<WantsAppServices> {
    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn without_services(self) -> Builder<WantsValidate> {
        Builder::<WantsValidate> {
            state: WantsValidate {
                parent: self.state,
                services: std::collections::BTreeMap::new(),
            },
        }
    }
}

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

        Config {
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
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::next::Config;

    #[test]
    fn build_simple() {
        let config = Config::builder()
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
