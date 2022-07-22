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
use super::wants::{
    WantsApp, WantsAppLogs, WantsAppVSL, WantsServer, WantsServerDNS, WantsServerInterfaces,
    WantsServerLogs, WantsServerQueues, WantsServerSMTPAuth, WantsServerSMTPConfig1,
    WantsServerSMTPConfig2, WantsServerSMTPConfig3, WantsServerSystem, WantsServerTLSConfig,
    WantsServerVirtual, WantsValidate, WantsVersion,
};
use crate::{
    field::{
        FieldApp, FieldAppLogs, FieldQueueDelivery, FieldQueueWorking, FieldServer, FieldServerDNS,
        FieldServerInterfaces, FieldServerLogs, FieldServerQueues, FieldServerSMTP,
        FieldServerSMTPAuth, FieldServerSMTPError, FieldServerSMTPTimeoutClient, FieldServerSystem,
        FieldServerSystemThreadPool, FieldServerTls, FieldServerVirtual, FieldServerVirtualTls,
        ResolverOptsWrapper, SecretFile, TlsSecurityLevel,
    },
    parser::{tls_certificate, tls_private_key},
};
use vsmtp_common::{
    auth::Mechanism,
    re::{
        anyhow::{self, Context},
        log,
    },
    state::StateSMTP,
    CodeID, Reply,
};

///
pub struct Builder<State> {
    pub(crate) state: State,
}

impl Builder<WantsVersion> {
    /// # Panics
    ///
    /// * `CARGO_PKG_VERSION` is not valid
    #[must_use]
    pub fn with_current_version(self) -> Builder<WantsServer> {
        self.with_version_str(env!("CARGO_PKG_VERSION")).unwrap()
    }

    /// # Errors
    ///
    /// * `version_requirement` is not valid format
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
    #[must_use]
    pub fn with_debug_server_info(self) -> Builder<WantsServerSystem> {
        self.with_server_name("debug.com")
    }

    ///
    #[must_use]
    pub fn with_hostname(self) -> Builder<WantsServerSystem> {
        self.with_hostname_and_client_count_max(FieldServer::default_client_count_max())
    }

    ///
    #[must_use]
    pub fn with_hostname_and_client_count_max(
        self,
        client_count_max: i64,
    ) -> Builder<WantsServerSystem> {
        self.with_server_name_and_client_count(&FieldServer::hostname(), client_count_max)
    }

    ///
    #[must_use]
    pub fn with_server_name(self, domain: &str) -> Builder<WantsServerSystem> {
        self.with_server_name_and_client_count(domain, FieldServer::default_client_count_max())
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
        self.with_system(
            FieldServerSystem::default_user(),
            FieldServerSystem::default_group(),
            None,
            FieldServerSystemThreadPool::default_receiver(),
            FieldServerSystemThreadPool::default_processing(),
            FieldServerSystemThreadPool::default_delivery(),
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
            FieldServerSystem::default_user(),
            FieldServerSystem::default_group(),
            None,
            thread_pool_receiver,
            thread_pool_processing,
            thread_pool_delivery,
        )
    }

    /// # Errors
    ///
    /// * `user` is not found
    /// * `group` is not found
    pub fn with_user_group_and_default_system(
        self,
        user: &str,
        group: &str,
    ) -> anyhow::Result<Builder<WantsServerInterfaces>> {
        self.with_system_str(
            user,
            group,
            None,
            FieldServerSystemThreadPool::default_receiver(),
            FieldServerSystemThreadPool::default_processing(),
            FieldServerSystemThreadPool::default_delivery(),
        )
    }

    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_system(
        self,
        user: users::User,
        group: users::Group,
        group_local: Option<users::Group>,
        thread_pool_receiver: usize,
        thread_pool_processing: usize,
        thread_pool_delivery: usize,
    ) -> Builder<WantsServerInterfaces> {
        Builder::<WantsServerInterfaces> {
            state: WantsServerInterfaces {
                parent: self.state,
                user,
                group,
                group_local,
                thread_pool_receiver,
                thread_pool_processing,
                thread_pool_delivery,
            },
        }
    }

    /// # Errors
    ///
    /// * `user` is not found
    /// * `group` is not found
    pub fn with_system_str(
        self,
        user: &str,
        group: &str,
        group_local: Option<&str>,
        thread_pool_receiver: usize,
        thread_pool_processing: usize,
        thread_pool_delivery: usize,
    ) -> anyhow::Result<Builder<WantsServerInterfaces>> {
        Ok(Builder::<WantsServerInterfaces> {
            state: WantsServerInterfaces {
                parent: self.state,
                user: users::get_user_by_name(user)
                    .ok_or_else(|| anyhow::anyhow!("user not found: '{}'", user))?,
                group: users::get_group_by_name(group)
                    .ok_or_else(|| anyhow::anyhow!("group not found: '{}'", group))?,
                group_local: if let Some(group_local) = group_local {
                    Some(
                        users::get_group_by_name(group_local)
                            .ok_or_else(|| anyhow::anyhow!("group not found: '{}'", group_local))?,
                    )
                } else {
                    None
                },
                thread_pool_receiver,
                thread_pool_processing,
                thread_pool_delivery,
            },
        })
    }
}

impl Builder<WantsServerInterfaces> {
    ///
    #[must_use]
    pub fn with_ipv4_localhost(self) -> Builder<WantsServerLogs> {
        let ipv4_localhost = FieldServerInterfaces::ipv4_localhost();
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
            FieldServerLogs::default_filepath(),
            FieldServerLogs::default_format(),
            FieldServerLogs::default_level(),
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
                size_limit: FieldServerLogs::default_size_limit(),
                archive_count: FieldServerLogs::default_archive_count(),
            },
        }
    }
}

impl Builder<WantsServerQueues> {
    ///
    #[must_use]
    pub fn with_default_delivery(self) -> Builder<WantsServerTLSConfig> {
        self.with_spool_dir_and_default_queues(FieldServerQueues::default_dirpath())
    }

    ///
    #[must_use]
    pub fn with_spool_dir_and_default_queues(
        self,
        spool_dir: impl Into<std::path::PathBuf>,
    ) -> Builder<WantsServerTLSConfig> {
        self.with_spool_dir_and_queues(
            spool_dir,
            FieldQueueWorking::default(),
            FieldQueueDelivery::default(),
        )
    }

    ///
    #[must_use]
    pub fn with_spool_dir_and_queues(
        self,
        spool_dir: impl Into<std::path::PathBuf>,
        working: FieldQueueWorking,
        delivery: FieldQueueDelivery,
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
    /// * `certificate` is not valid
    /// * `private_key` is not valid
    pub fn with_safe_tls_config(
        self,
        certificate: &str,
        private_key: &str,
    ) -> anyhow::Result<Builder<WantsServerSMTPConfig1>> {
        Ok(Builder::<WantsServerSMTPConfig1> {
            state: WantsServerSMTPConfig1 {
                parent: self.state,
                tls: Some(FieldServerTls {
                    security_level: TlsSecurityLevel::May,
                    preempt_cipherlist: false,
                    handshake_timeout: std::time::Duration::from_millis(200),
                    protocol_version: vec![rustls::ProtocolVersion::TLSv1_3],
                    certificate: SecretFile::<rustls::Certificate> {
                        inner: tls_certificate::from_string(certificate)?,
                        path: certificate.into(),
                    },
                    private_key: SecretFile::<rustls::PrivateKey> {
                        inner: tls_private_key::from_string(private_key)?,
                        path: private_key.into(),
                    },
                    cipher_suite: FieldServerTls::default_cipher_suite(),
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
    #[must_use]
    pub fn with_default_smtp_options(self) -> Builder<WantsServerSMTPConfig2> {
        self.with_rcpt_count_and_default(FieldServerSMTP::default_rcpt_count_max())
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
                disable_ehlo: FieldServerSMTP::default_disable_ehlo(),
                required_extension: FieldServerSMTP::default_required_extension(),
            },
        }
    }
}

impl Builder<WantsServerSMTPConfig2> {
    ///
    #[must_use]
    pub fn with_default_smtp_error_handler(self) -> Builder<WantsServerSMTPConfig3> {
        Builder::<WantsServerSMTPConfig3> {
            state: WantsServerSMTPConfig3 {
                parent: self.state,
                error: FieldServerSMTPError::default(),
                timeout_client: FieldServerSMTPTimeoutClient::default(),
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
                error: FieldServerSMTPError {
                    soft_count,
                    hard_count,
                    delay,
                },
                timeout_client: FieldServerSMTPTimeoutClient {
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
    pub fn with_default_smtp_codes(self) -> Builder<WantsServerSMTPAuth> {
        self.with_smtp_codes(std::collections::BTreeMap::new())
    }

    ///
    #[must_use]
    pub fn with_smtp_codes(
        self,
        codes: std::collections::BTreeMap<CodeID, Reply>,
    ) -> Builder<WantsServerSMTPAuth> {
        Builder::<WantsServerSMTPAuth> {
            state: WantsServerSMTPAuth {
                parent: self.state,
                codes,
            },
        }
    }
}

impl Builder<WantsServerSMTPAuth> {
    ///
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn without_auth(self) -> Builder<WantsApp> {
        Builder::<WantsApp> {
            state: WantsApp {
                parent: self.state,
                auth: None,
            },
        }
    }

    ///
    #[must_use]
    pub fn with_safe_auth(
        self,
        must_be_authenticated: bool,
        attempt_count_max: i64,
    ) -> Builder<WantsApp> {
        self.with_auth(
            must_be_authenticated,
            FieldServerSMTPAuth::default_enable_dangerous_mechanism_in_clair(),
            FieldServerSMTPAuth::default_mechanisms(),
            attempt_count_max,
        )
    }

    ///
    #[must_use]
    pub fn with_auth(
        self,
        must_be_authenticated: bool,
        enable_dangerous_mechanism_in_clair: bool,
        mechanisms: Vec<Mechanism>,
        attempt_count_max: i64,
    ) -> Builder<WantsApp> {
        Builder::<WantsApp> {
            state: WantsApp {
                parent: self.state,
                auth: Some(FieldServerSMTPAuth {
                    must_be_authenticated,
                    enable_dangerous_mechanism_in_clair,
                    mechanisms,
                    attempt_count_max,
                }),
            },
        }
    }
}

impl Builder<WantsApp> {
    ///
    #[must_use]
    pub fn with_default_app(self) -> Builder<WantsAppVSL> {
        self.with_app_at_location(FieldApp::default_dirpath())
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
    #[allow(clippy::missing_const_for_fn)]
    pub fn with_default_vsl_settings(self) -> Builder<WantsAppLogs> {
        Builder::<WantsAppLogs> {
            state: WantsAppLogs {
                parent: self.state,
                filepath: None,
            },
        }
    }

    ///
    #[must_use]
    pub fn with_vsl(self, entry_point: impl Into<std::path::PathBuf>) -> Builder<WantsAppLogs> {
        Builder::<WantsAppLogs> {
            state: WantsAppLogs {
                parent: self.state,
                filepath: Some(entry_point.into()),
            },
        }
    }
}

impl Builder<WantsAppLogs> {
    ///
    #[must_use]
    pub fn with_default_app_logs(self) -> Builder<WantsServerDNS> {
        self.with_app_logs_at(FieldAppLogs::default_filepath())
    }

    ///
    #[must_use]
    pub fn with_app_logs_at(
        self,
        filepath: impl Into<std::path::PathBuf>,
    ) -> Builder<WantsServerDNS> {
        self.with_app_logs_level_and_format(
            filepath,
            FieldAppLogs::default_level(),
            FieldAppLogs::default_format(),
            FieldAppLogs::default_size_limit(),
            FieldAppLogs::default_archive_count(),
        )
    }

    ///
    #[must_use]
    pub fn with_app_logs_level_and_format(
        self,
        filepath: impl Into<std::path::PathBuf>,
        level: log::LevelFilter,
        format: impl Into<String>,
        size_limit: u64,
        archive_count: u32,
    ) -> Builder<WantsServerDNS> {
        Builder::<WantsServerDNS> {
            state: WantsServerDNS {
                parent: self.state,
                filepath: filepath.into(),
                level,
                format: format.into(),
                size_limit,
                archive_count,
            },
        }
    }
}

impl Builder<WantsServerDNS> {
    /// dns resolutions will be made using google's service.
    #[must_use]
    pub fn with_google_dns(self) -> Builder<WantsServerVirtual> {
        Builder::<WantsServerVirtual> {
            state: WantsServerVirtual {
                parent: self.state,
                config: FieldServerDNS::Google {
                    options: ResolverOptsWrapper::default(),
                },
            },
        }
    }

    /// dns resolutions will be made using couldflare's service.
    #[must_use]
    pub fn with_cloudflare_dns(self) -> Builder<WantsServerVirtual> {
        Builder::<WantsServerVirtual> {
            state: WantsServerVirtual {
                parent: self.state,
                config: FieldServerDNS::CloudFlare {
                    options: ResolverOptsWrapper::default(),
                },
            },
        }
    }

    /// dns resolutions will be made using the system configuration.
    /// (/etc/resolv.conf on unix systems & the registry on Windows).
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_system_dns(self) -> Builder<WantsServerVirtual> {
        Builder::<WantsServerVirtual> {
            state: WantsServerVirtual {
                parent: self.state,
                config: FieldServerDNS::System,
            },
        }
    }

    /// dns resolutions will be made using the following dns configuration.
    #[allow(clippy::missing_const_for_fn)]
    #[must_use]
    pub fn with_dns(
        self,
        config: trust_dns_resolver::config::ResolverConfig,
        options: ResolverOptsWrapper,
    ) -> Builder<WantsServerVirtual> {
        Builder::<WantsServerVirtual> {
            state: WantsServerVirtual {
                parent: self.state,
                config: FieldServerDNS::Custom { config, options },
            },
        }
    }
}

/// metadata for a virtual entry.
pub struct VirtualEntry {
    /// the domain of the entry.
    pub domain: String,
    /// path to the certificate and private key used for tls.
    pub tls: Option<(String, String)>,
    /// dns configuration.
    pub dns: Option<FieldServerDNS>,
}

impl Builder<WantsServerVirtual> {
    ///
    #[must_use]
    pub fn without_virtual_entries(self) -> Builder<WantsValidate> {
        Builder::<WantsValidate> {
            state: WantsValidate {
                parent: self.state,
                r#virtual: std::collections::BTreeMap::new(),
            },
        }
    }

    /// adds multiple virtual entries to the server.
    ///
    /// # Errors
    ///
    /// * one of the certificate is not valid
    /// * one private key is not valid
    pub fn with_virtual_entries(
        self,
        entries: &[VirtualEntry],
    ) -> anyhow::Result<Builder<WantsValidate>> {
        let mut r#virtual = std::collections::BTreeMap::new();

        for entry in entries {
            r#virtual.insert(
                entry.domain.clone(),
                match (entry.tls.as_ref(), entry.dns.clone()) {
                    (None, None) => FieldServerVirtual {
                        tls: None,
                        dns: None,
                        dkim: None,
                    },
                    (None, Some(dns_config)) => FieldServerVirtual {
                        tls: None,
                        dns: Some(dns_config),
                        dkim: None,
                    },
                    // ::with_dns(dns_config.clone())?,
                    (Some((certificate, private_key)), None) => FieldServerVirtual {
                        tls: Some(FieldServerVirtualTls::from_path(certificate, private_key)?),
                        dns: None,
                        dkim: None,
                    },
                    (Some((certificate, private_key)), Some(dns_config)) => FieldServerVirtual {
                        tls: Some(FieldServerVirtualTls::from_path(certificate, private_key)?),
                        dns: Some(dns_config),
                        dkim: None,
                    },
                },
            );
        }

        Ok(Builder::<WantsValidate> {
            state: WantsValidate {
                parent: self.state,
                r#virtual,
            },
        })
    }
}
