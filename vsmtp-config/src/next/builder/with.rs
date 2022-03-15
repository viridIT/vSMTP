use crate::next::config::{
    ConfigQueueDelivery, ConfigQueueWorking, ConfigServerSMTPError, ConfigServerSMTPTimeoutClient,
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
                security_level: crate::TlsSecurityLevel::May,
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
