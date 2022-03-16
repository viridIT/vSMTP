use anyhow::Context;

use crate::{
    config::{
        ConfigQueueDelivery, ConfigQueueWorking, ConfigServerSMTPError,
        ConfigServerSMTPTimeoutClient, ConfigServerTls, TlsSecurityLevel,
    },
    parser::{tls_certificate, tls_private_key},
    Service,
};

use super::wants::{
    WantsApp, WantsAppLogs, WantsAppServices, WantsAppVSL, WantsServer, WantsServerInterfaces,
    WantsServerLogs, WantsServerQueues, WantsServerSMTPConfig1, WantsServerSMTPConfig2,
    WantsServerSMTPConfig3, WantsServerSystem, WantsServerTLSConfig, WantsValidate, WantsVersion,
};

///
pub struct Builder<State> {
    pub(crate) state: State,
}

impl Builder<WantsVersion> {
    ///
    ///
    /// # Panics
    ///
    /// * CARGO_PKG_VERSION is not valid
    #[must_use]
    pub fn with_current_version(self) -> Builder<WantsServer> {
        self.with_version_str(env!("CARGO_PKG_VERSION")).unwrap()
    }

    ///
    ///
    /// # Errors
    ///
    /// * version_requirement is not valid format
    pub fn with_version_str(
        self,
        version_requirement: &str,
    ) -> anyhow::Result<Builder<WantsServer>> {
        semver::VersionReq::parse(version_requirement)
            .with_context(|| format!("version is not valid: '{version_requirement}'"))
            .map(|version_requirement| Builder::<WantsServer> {
                state: WantsServer {
                    parent: self.state,
                    version_requirement,
                },
            })
    }
}

impl Builder<WantsServer> {
    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_debug_server_info(self) -> Builder<WantsServerSystem> {
        self.with_server_name("debug.com")
    }

    ///
    #[must_use]
    pub fn with_server_name(self, domain: &str) -> Builder<WantsServerSystem> {
        self.with_server_name_and_client_count(domain, 16)
    }

    ///
    #[must_use]
    pub fn with_server_name_and_client_count(
        self,
        domain: &str,
        client_count_max: i64,
    ) -> Builder<WantsServerSystem> {
        Builder::<WantsServerSystem> {
            state: WantsServerSystem {
                parent: self.state,
                domain: domain.to_string(),
                client_count_max,
            },
        }
    }
}

impl Builder<WantsServerSystem> {
    ///
    #[must_use]
    pub fn with_default_system(self) -> Builder<WantsServerInterfaces> {
        self.with_user_group_and_default_system("vsmtp", "vsmtp")
    }

    ///
    #[must_use]
    pub fn with_user_group_and_default_system(
        self,
        user: &str,
        group: &str,
    ) -> Builder<WantsServerInterfaces> {
        Builder::<WantsServerInterfaces> {
            state: WantsServerInterfaces {
                parent: self.state,
                user: user.to_string(),
                group: group.to_string(),
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
        self.with_interfaces(
            &["0.0.0.0:25".parse().expect("valid")],
            &["0.0.0.0:587".parse().expect("valid")],
            &["0.0.0.0:465".parse().expect("valid")],
        )
    }

    ///
    #[must_use]
    pub fn with_interfaces(
        self,
        addr: &[std::net::SocketAddr],
        addr_submission: &[std::net::SocketAddr],
        addr_submissions: &[std::net::SocketAddr],
    ) -> Builder<WantsServerLogs> {
        Builder::<WantsServerLogs> {
            state: WantsServerLogs {
                parent: self.state,
                addr: addr.to_vec(),
                addr_submission: addr_submission.to_vec(),
                addr_submissions: addr_submissions.to_vec(),
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
    pub fn with_default_delivery(self) -> Builder<WantsServerTLSConfig> {
        self.with_spool_dir_and_default_queues("/var/spool/vsmtp")
    }

    ///
    #[must_use]
    pub fn with_spool_dir_and_default_queues(
        self,
        spool_dir: &str,
    ) -> Builder<WantsServerTLSConfig> {
        Builder::<WantsServerTLSConfig> {
            state: WantsServerTLSConfig {
                parent: self.state,
                dirpath: spool_dir.into(),
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
    /// # Errors
    ///
    /// * certificate is not valid
    /// * private_key is not valid
    pub fn with_safe_tls_config(
        self,
        certificate: &str,
        private_key: &str,
    ) -> anyhow::Result<Builder<WantsServerSMTPConfig1>> {
        Ok(Builder::<WantsServerSMTPConfig1> {
            state: WantsServerSMTPConfig1 {
                parent: self.state,
                tls: Some(ConfigServerTls {
                    security_level: TlsSecurityLevel::May,
                    preempt_cipherlist: false,
                    handshake_timeout: std::time::Duration::from_millis(200),
                    protocol_version: vec![rustls::ProtocolVersion::TLSv1_3],
                    certificate: tls_certificate::from_string(certificate)?,
                    private_key: tls_private_key::from_string(private_key)?,
                    sni: vec![],
                }),
            },
        })
    }

    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn without_tls_support(self) -> Builder<WantsServerSMTPConfig1> {
        Builder::<WantsServerSMTPConfig1> {
            state: WantsServerSMTPConfig1 {
                parent: self.state,
                tls: None,
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
        self.with_app_at_location("/var/spool/vsmtp/app")
    }

    ///
    #[must_use]
    pub fn with_app_at_location(self, dirpath: &str) -> Builder<WantsAppVSL> {
        Builder::<WantsAppVSL> {
            state: WantsAppVSL {
                parent: self.state,
                dirpath: dirpath.into(),
            },
        }
    }
}

impl Builder<WantsAppVSL> {
    ///
    #[must_use]
    pub fn with_default_vsl_settings(self) -> Builder<WantsAppLogs> {
        self.with_vsl("/etc/vsmtp/main.vsl")
    }

    ///
    #[must_use]
    pub fn with_vsl(self, entry_point: &str) -> Builder<WantsAppLogs> {
        Builder::<WantsAppLogs> {
            state: WantsAppLogs {
                parent: self.state,
                filepath: entry_point.into(),
            },
        }
    }
}

impl Builder<WantsAppLogs> {
    ///
    #[must_use]
    pub fn with_default_app_logs(self) -> Builder<WantsAppServices> {
        self.with_app_logs("/var/log/vsmtp/app.log")
    }

    ///
    #[must_use]
    pub fn with_app_logs(self, filepath: &str) -> Builder<WantsAppServices> {
        Builder::<WantsAppServices> {
            state: WantsAppServices {
                parent: self.state,
                filepath: filepath.into(),
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
        self.with_services(std::collections::BTreeMap::new())
    }

    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_services(
        self,
        services: std::collections::BTreeMap<String, Service>,
    ) -> Builder<WantsValidate> {
        Builder::<WantsValidate> {
            state: WantsValidate {
                parent: self.state,
                services,
            },
        }
    }
}
