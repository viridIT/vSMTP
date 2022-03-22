use anyhow::Context;
use vsmtp_common::{code::SMTPReplyCode, state::StateSMTP};

use crate::{
    config::{
        ConfigApp, ConfigAppLogs, ConfigAppVSL, ConfigDNS, ConfigDeliveryTarget,
        ConfigQueueDelivery, ConfigQueueWorking, ConfigServer, ConfigServerInterfaces,
        ConfigServerLogs, ConfigServerQueues, ConfigServerSMTP, ConfigServerSMTPError,
        ConfigServerSMTPTimeoutClient, ConfigServerSystem, ConfigServerSystemThreadPool,
        ConfigServerTls, ConfigServerTlsSni, TlsSecurityLevel,
    },
    parser::{tls_certificate, tls_private_key},
    Service, WantsDeliveryTargets, WantsServerDNS,
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
    pub fn with_hostname(self) -> Builder<WantsServerSystem> {
        self.with_hostname_and_client_count_max(ConfigServer::default_client_count_max())
    }

    ///
    #[must_use]
    pub fn with_hostname_and_client_count_max(
        self,
        client_count_max: i64,
    ) -> Builder<WantsServerSystem> {
        self.with_server_name_and_client_count(&ConfigServer::hostname(), client_count_max)
    }

    ///
    #[must_use]
    pub fn with_server_name(self, domain: &str) -> Builder<WantsServerSystem> {
        self.with_server_name_and_client_count(domain, ConfigServer::default_client_count_max())
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
        self.with_user_group_and_default_system(
            &ConfigServerSystem::default_user(),
            &ConfigServerSystem::default_group(),
        )
    }

    ///
    #[must_use]
    pub fn with_default_user_and_thread_pool(
        self,
        thread_pool_receiver: usize,
        thread_pool_processing: usize,
        thread_pool_delivery: usize,
    ) -> Builder<WantsServerInterfaces> {
        self.with_system(
            &ConfigServerSystem::default_user(),
            &ConfigServerSystem::default_group(),
            thread_pool_receiver,
            thread_pool_processing,
            thread_pool_delivery,
        )
    }

    ///
    #[must_use]
    pub fn with_user_group_and_default_system(
        self,
        user: &str,
        group: &str,
    ) -> Builder<WantsServerInterfaces> {
        self.with_system(
            user,
            group,
            ConfigServerSystemThreadPool::default_receiver(),
            ConfigServerSystemThreadPool::default_processing(),
            ConfigServerSystemThreadPool::default_delivery(),
        )
    }

    ///
    #[must_use]
    pub fn with_system(
        self,
        user: &str,
        group: &str,
        thread_pool_receiver: usize,
        thread_pool_processing: usize,
        thread_pool_delivery: usize,
    ) -> Builder<WantsServerInterfaces> {
        Builder::<WantsServerInterfaces> {
            state: WantsServerInterfaces {
                parent: self.state,
                user: user.to_string(),
                group: group.to_string(),
                thread_pool_receiver,
                thread_pool_processing,
                thread_pool_delivery,
            },
        }
    }
}

impl Builder<WantsServerInterfaces> {
    ///
    #[must_use]
    pub fn with_ipv4_localhost(self) -> Builder<WantsServerLogs> {
        let ipv4_localhost = ConfigServerInterfaces::ipv4_localhost();
        self.with_interfaces(
            &ipv4_localhost.addr,
            &ipv4_localhost.addr_submission,
            &ipv4_localhost.addr_submissions,
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
    pub fn with_default_logs_settings(self) -> Builder<WantsServerQueues> {
        self.with_logs_settings(
            ConfigServerLogs::default_filepath(),
            ConfigServerLogs::default_format(),
            ConfigServerLogs::default_level(),
        )
    }

    ///
    #[must_use]
    pub fn with_logs_settings(
        self,
        filepath: impl Into<std::path::PathBuf>,
        format: impl Into<String>,
        level: std::collections::BTreeMap<String, log::LevelFilter>,
    ) -> Builder<WantsServerQueues> {
        Builder::<WantsServerQueues> {
            state: WantsServerQueues {
                parent: self.state,
                filepath: filepath.into(),
                format: format.into(),
                level,
            },
        }
    }
}

impl Builder<WantsServerQueues> {
    ///
    #[must_use]
    pub fn with_default_delivery(self) -> Builder<WantsServerTLSConfig> {
        self.with_spool_dir_and_default_queues(ConfigServerQueues::default_dirpath())
    }

    ///
    #[must_use]
    pub fn with_spool_dir_and_default_queues(
        self,
        spool_dir: impl Into<std::path::PathBuf>,
    ) -> Builder<WantsServerTLSConfig> {
        self.with_spool_dir_and_queues(
            spool_dir,
            ConfigQueueWorking::default(),
            ConfigQueueDelivery::default(),
        )
    }

    ///
    #[must_use]
    pub fn with_spool_dir_and_queues(
        self,
        spool_dir: impl Into<std::path::PathBuf>,
        working: ConfigQueueWorking,
        delivery: ConfigQueueDelivery,
    ) -> Builder<WantsServerTLSConfig> {
        Builder::<WantsServerTLSConfig> {
            state: WantsServerTLSConfig {
                parent: self.state,
                dirpath: spool_dir.into(),
                working,
                delivery,
            },
        }
    }
}

impl Builder<WantsServerTLSConfig> {
    // TODO: remove default values from this files
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
    /// # Errors
    ///
    /// * certificate is not valid
    /// * private_key is not valid
    pub fn with_sni_entry(
        self,
        domain: &str,
        certificate: &str,
        private_key: &str,
    ) -> anyhow::Result<Self> {
        let mut tls = self
            .state
            .tls
            .ok_or_else(|| anyhow::anyhow!("sni can only be used with tls"))?;
        Ok(Self {
            state: WantsServerSMTPConfig1 {
                parent: self.state.parent,
                tls: Some(ConfigServerTls {
                    sni: {
                        tls.sni.push(ConfigServerTlsSni {
                            domain: domain.to_string(),
                            certificate: tls_certificate::from_string(certificate)?,
                            private_key: tls_private_key::from_string(private_key)?,
                        });
                        tls.sni
                    },
                    ..tls
                }),
            },
        })
    }

    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_default_smtp_options(self) -> Builder<WantsServerSMTPConfig2> {
        self.with_rcpt_count_and_default(ConfigServerSMTP::default_rcpt_count_max())
    }

    ///
    #[must_use]
    pub fn with_rcpt_count_and_default(
        self,
        rcpt_count_max: usize,
    ) -> Builder<WantsServerSMTPConfig2> {
        Builder::<WantsServerSMTPConfig2> {
            state: WantsServerSMTPConfig2 {
                parent: self.state,
                rcpt_count_max,
                disable_ehlo: ConfigServerSMTP::default_disable_ehlo(),
                required_extension: ConfigServerSMTP::default_required_extension(),
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
                error: ConfigServerSMTPError::default(),
                timeout_client: ConfigServerSMTPTimeoutClient::default(),
            },
        }
    }

    // TODO: remove default values from this files
    ///
    #[must_use]
    pub fn with_error_handler_and_timeout(
        self,
        soft_count: i64,
        hard_count: i64,
        delay: std::time::Duration,
        timeout_client: &std::collections::BTreeMap<StateSMTP, std::time::Duration>,
    ) -> Builder<WantsServerSMTPConfig3> {
        Builder::<WantsServerSMTPConfig3> {
            state: WantsServerSMTPConfig3 {
                parent: self.state,
                error: ConfigServerSMTPError {
                    soft_count,
                    hard_count,
                    delay,
                },
                timeout_client: ConfigServerSMTPTimeoutClient {
                    connect: *timeout_client
                        .get(&StateSMTP::Connect)
                        .unwrap_or(&std::time::Duration::from_millis(1000)),
                    helo: *timeout_client
                        .get(&StateSMTP::Helo)
                        .unwrap_or(&std::time::Duration::from_millis(1000)),
                    mail_from: *timeout_client
                        .get(&StateSMTP::MailFrom)
                        .unwrap_or(&std::time::Duration::from_millis(1000)),
                    rcpt_to: *timeout_client
                        .get(&StateSMTP::RcptTo)
                        .unwrap_or(&std::time::Duration::from_millis(1000)),
                    data: *timeout_client
                        .get(&StateSMTP::Data)
                        .unwrap_or(&std::time::Duration::from_millis(1000)),
                },
            },
        }
    }
}

impl Builder<WantsServerSMTPConfig3> {
    ///
    #[must_use]
    pub fn with_default_smtp_codes(self) -> Builder<WantsApp> {
        self.with_smtp_codes(std::collections::BTreeMap::new())
    }

    ///
    #[must_use]
    pub fn with_smtp_codes(
        self,
        codes: std::collections::BTreeMap<SMTPReplyCode, String>,
    ) -> Builder<WantsApp> {
        Builder::<WantsApp> {
            state: WantsApp {
                parent: self.state,
                codes,
            },
        }
    }
}

impl Builder<WantsApp> {
    ///
    #[must_use]
    pub fn with_default_app(self) -> Builder<WantsAppVSL> {
        self.with_app_at_location(ConfigApp::default_dirpath())
    }

    ///
    #[must_use]
    pub fn with_app_at_location(
        self,
        dirpath: impl Into<std::path::PathBuf>,
    ) -> Builder<WantsAppVSL> {
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
        self.with_vsl(ConfigAppVSL::default_filepath())
    }

    ///
    #[must_use]
    pub fn with_vsl(self, entry_point: impl Into<std::path::PathBuf>) -> Builder<WantsAppLogs> {
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
        self.with_app_logs(ConfigAppLogs::default_filepath())
    }

    ///
    #[must_use]
    pub fn with_app_logs(
        self,
        filepath: impl Into<std::path::PathBuf>,
    ) -> Builder<WantsAppServices> {
        self.with_app_logs_level_and_format(
            filepath,
            ConfigAppLogs::default_level(),
            ConfigAppLogs::default_format(),
        )
    }

    ///
    #[must_use]
    pub fn with_app_logs_level_and_format(
        self,
        filepath: impl Into<std::path::PathBuf>,
        level: log::LevelFilter,
        format: impl Into<String>,
    ) -> Builder<WantsAppServices> {
        Builder::<WantsAppServices> {
            state: WantsAppServices {
                parent: self.state,
                filepath: filepath.into(),
                level,
                format: format.into(),
            },
        }
    }
}

impl Builder<WantsAppServices> {
    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn without_services(self) -> Builder<WantsServerDNS> {
        self.with_services(std::collections::BTreeMap::new())
    }

    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_services(
        self,
        services: std::collections::BTreeMap<String, Service>,
    ) -> Builder<WantsServerDNS> {
        Builder::<WantsServerDNS> {
            state: WantsServerDNS {
                parent: self.state,
                services,
            },
        }
    }
}

impl Builder<WantsServerDNS> {
    /// dns resolutions will be made using google's service.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_google_dns(self) -> Builder<WantsDeliveryTargets> {
        Builder::<WantsDeliveryTargets> {
            state: WantsDeliveryTargets {
                parent: self.state,
                config: ConfigDNS::Google,
            },
        }
    }

    /// dns resolutions will be made using couldflare's service.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_cloudflare_dns(self) -> Builder<WantsDeliveryTargets> {
        Builder::<WantsDeliveryTargets> {
            state: WantsDeliveryTargets {
                parent: self.state,
                config: ConfigDNS::CloudFlare,
            },
        }
    }

    /// dns resolutions will be made using the system configuration.
    /// (/etc/resolv.conf on unix systems & the registry on Windows).
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_system_dns(self) -> Builder<WantsDeliveryTargets> {
        Builder::<WantsDeliveryTargets> {
            state: WantsDeliveryTargets {
                parent: self.state,
                config: ConfigDNS::System,
            },
        }
    }

    /// dns resolutions will be made using the following dns configuration.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_dns(
        self,
        config: trust_dns_resolver::config::ResolverConfig,
        options: trust_dns_resolver::config::ResolverOpts,
    ) -> Builder<WantsDeliveryTargets> {
        Builder::<WantsDeliveryTargets> {
            state: WantsDeliveryTargets {
                parent: self.state,
                config: ConfigDNS::Custom { config, options },
            },
        }
    }
}

impl Builder<WantsDeliveryTargets> {
    /// emails will be delivered using the default transport.
    #[must_use]
    pub fn without_delivery_targets(self) -> Builder<WantsValidate> {
        Builder::<WantsValidate> {
            state: WantsValidate {
                parent: self.state,
                delivery_targets: std::collections::HashMap::new(),
            },
        }
    }

    /// emails will be delivered using the targets passed as arguments.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_delivery_targets(
        self,
        delivery_targets: std::collections::HashMap<String, ConfigDeliveryTarget>,
    ) -> Builder<WantsValidate> {
        Builder::<WantsValidate> {
            state: WantsValidate {
                parent: self.state,
                delivery_targets,
            },
        }
    }
}
