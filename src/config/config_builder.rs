/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
use crate::smtp::{code::SMTPReplyCode, state::StateSMTP};

use super::{
    server_config::{
        Codes, DurationAlias, InnerDeliveryConfig, InnerLogConfig, InnerRulesConfig,
        InnerSMTPConfig, InnerSMTPErrorConfig, InnerServerConfig, InnerSmtpsConfig,
        InnerUserLogConfig, ProtocolVersion, ProtocolVersionRequirement, QueueConfig, ServerConfig,
        SniKey, TlsSecurityLevel,
    },
    service::Service,
};

pub struct ConfigBuilder<State> {
    pub(crate) state: State,
}

impl ServerConfig {
    /// Return an instance of [ConfigBuilder] for a step-by-step configuration generation
    pub fn builder() -> ConfigBuilder<WantsVersion> {
        ConfigBuilder {
            state: WantsVersion(()),
        }
    }

    /// Parse a [ServerConfig] with [TOML] format
    ///
    /// Produce an error if :
    /// * file is not a valid [TOML]
    /// * one field is unknown
    /// * the version requirement are not fulfilled
    /// * a mandatory field is not provided (no default value)
    ///
    /// [TOML]: https://github.com/toml-lang/toml
    pub fn from_toml(data: &str) -> anyhow::Result<ServerConfig> {
        let parsed_ahead = ConfigBuilder::<WantsServer> {
            state: toml::from_str::<WantsServer>(data)?,
        };
        let pkg_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;

        if !parsed_ahead.state.version_requirement.matches(&pkg_version) {
            anyhow::bail!(
                "Version requirement not fulfilled: expected '{}' but got '{}'",
                parsed_ahead.state.version_requirement,
                env!("CARGO_PKG_VERSION")
            );
        }

        ConfigBuilder::<WantsBuild> {
            state: toml::from_str::<WantsBuild>(data)?,
        }
        .build()
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct WantsVersion(pub(crate) ());

impl ConfigBuilder<WantsVersion> {
    pub fn with_version(
        self,
        version_requirement: semver::VersionReq,
    ) -> ConfigBuilder<WantsServer> {
        ConfigBuilder::<WantsServer> {
            state: WantsServer {
                parent: self.state,
                version_requirement,
            },
        }
    }

    pub fn with_version_str(
        self,
        version_requirement: &str,
    ) -> anyhow::Result<ConfigBuilder<WantsServer>> {
        Ok(ConfigBuilder::<WantsServer> {
            state: WantsServer {
                parent: self.state,
                version_requirement: semver::VersionReq::parse(version_requirement)?,
            },
        })
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct WantsServer {
    #[serde(skip)]
    #[allow(unused)]
    pub(crate) parent: WantsVersion,
    #[serde(
        serialize_with = "crate::config::serializer::serialize_version_req",
        deserialize_with = "crate::config::serializer::deserialize_version_req"
    )]
    version_requirement: semver::VersionReq,
}

impl ConfigBuilder<WantsServer> {
    #[allow(clippy::too_many_arguments)]
    pub fn with_server(
        self,
        domain: impl Into<String>,
        vsmtp_user: impl Into<String>,
        vsmtp_group: impl Into<String>,
        addr: std::net::SocketAddr,
        addr_submission: std::net::SocketAddr,
        addr_submissions: std::net::SocketAddr,
        thread_count: usize,
    ) -> ConfigBuilder<WantsLogging> {
        ConfigBuilder::<WantsLogging> {
            state: WantsLogging {
                parent: self.state,
                server: InnerServerConfig {
                    domain: domain.into(),
                    addr,
                    addr_submission,
                    addr_submissions,
                    thread_count,
                    vsmtp_user: vsmtp_user.into(),
                    vsmtp_group: vsmtp_group.into(),
                },
            },
        }
    }

    pub fn with_rfc_port(
        self,
        domain: impl Into<String>,
        vsmtp_user: impl Into<String>,
        vsmtp_group: impl Into<String>,
        thread_count: Option<usize>,
    ) -> ConfigBuilder<WantsLogging> {
        self.with_server(
            domain,
            vsmtp_user,
            vsmtp_group,
            "0.0.0.0:25".parse().expect("valid address"),
            "0.0.0.0:587".parse().expect("valid address"),
            "0.0.0.0:465".parse().expect("valid address"),
            thread_count.unwrap_or_else(num_cpus::get),
        )
    }

    pub fn with_debug_port(
        self,
        domain: impl Into<String>,
        vsmtp_user: impl Into<String>,
        vsmtp_group: impl Into<String>,
        thread_count: Option<usize>,
    ) -> ConfigBuilder<WantsLogging> {
        self.with_server(
            domain,
            vsmtp_user,
            vsmtp_group,
            "0.0.0.0:10025".parse().expect("valid address"),
            "0.0.0.0:10587".parse().expect("valid address"),
            "0.0.0.0:10465".parse().expect("valid address"),
            thread_count.unwrap_or_else(num_cpus::get),
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WantsLogging {
    #[serde(flatten)]
    pub(crate) parent: WantsServer,
    pub(crate) server: InnerServerConfig,
}

impl ConfigBuilder<WantsLogging> {
    pub fn with_logging(
        self,
        file: impl Into<std::path::PathBuf>,
        level: std::collections::HashMap<String, log::LevelFilter>,
    ) -> ConfigBuilder<WantSMTPS> {
        ConfigBuilder::<WantSMTPS> {
            state: WantSMTPS {
                parent: self.state,
                logs: InnerLogConfig {
                    file: file.into(),
                    level,
                },
            },
        }
    }

    pub fn without_log(self) -> ConfigBuilder<WantSMTPS> {
        self.with_logging(
            "./tmp/trash/log.log",
            crate::collection! {
                "default".to_string() => log::LevelFilter::Off
            },
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WantSMTPS {
    #[serde(flatten)]
    pub(crate) parent: WantsLogging,
    pub(crate) logs: InnerLogConfig,
}

impl ConfigBuilder<WantSMTPS> {
    #[allow(clippy::too_many_arguments)]
    pub fn with_smtps(
        self,
        security_level: TlsSecurityLevel,
        protocol_version: ProtocolVersionRequirement,
        capath: impl Into<std::path::PathBuf>,
        preempt_cipherlist: bool,
        fullchain: impl Into<std::path::PathBuf>,
        private_key: impl Into<std::path::PathBuf>,
        handshake_timeout: std::time::Duration,
        sni_maps: Option<Vec<SniKey>>,
    ) -> ConfigBuilder<WantSMTP> {
        ConfigBuilder::<WantSMTP> {
            state: WantSMTP {
                parent: self.state,
                smtps: Some(InnerSmtpsConfig {
                    security_level,
                    protocol_version,
                    capath: capath.into(),
                    preempt_cipherlist,
                    fullchain: fullchain.into(),
                    private_key: private_key.into(),
                    handshake_timeout,
                    sni_maps,
                }),
            },
        }
    }

    pub fn with_safe_default_smtps(
        self,
        security_level: TlsSecurityLevel,
        fullchain: impl Into<std::path::PathBuf>,
        private_key: impl Into<std::path::PathBuf>,
        sni_maps: Option<Vec<SniKey>>,
    ) -> ConfigBuilder<WantSMTP> {
        self.with_smtps(
            security_level,
            ProtocolVersionRequirement(vec![
                ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
                ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
            ]),
            "./certs",
            true,
            fullchain,
            private_key,
            std::time::Duration::from_millis(100),
            sni_maps,
        )
    }

    pub fn without_smtps(self) -> ConfigBuilder<WantSMTP> {
        ConfigBuilder::<WantSMTP> {
            state: WantSMTP {
                parent: self.state,
                smtps: None,
            },
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WantSMTP {
    #[serde(flatten)]
    pub(crate) parent: WantSMTPS,
    pub(crate) smtps: Option<InnerSmtpsConfig>,
}

impl ConfigBuilder<WantSMTP> {
    #[allow(clippy::too_many_arguments)]
    pub fn with_smtp(
        self,
        disable_ehlo: bool,
        timeout_client: std::collections::HashMap<StateSMTP, std::time::Duration>,
        error_soft_count: i64,
        error_hard_count: i64,
        error_delay: std::time::Duration,
        rcpt_count_max: usize,
        client_count_max: i64,
    ) -> ConfigBuilder<WantsDelivery> {
        ConfigBuilder::<WantsDelivery> {
            state: WantsDelivery {
                parent: self.state,
                smtp: InnerSMTPConfig {
                    disable_ehlo,
                    timeout_client: timeout_client
                        .into_iter()
                        .map(|(k, v)| (k, DurationAlias { alias: v }))
                        .collect(),
                    error: InnerSMTPErrorConfig {
                        soft_count: error_soft_count,
                        hard_count: error_hard_count,
                        delay: error_delay,
                    },
                    rcpt_count_max,
                    client_count_max,
                },
            },
        }
    }

    pub fn with_default_smtp(self) -> ConfigBuilder<WantsDelivery> {
        self.with_smtp(
            false,
            crate::collection! {},
            5,
            10,
            std::time::Duration::from_millis(1000),
            1000,
            InnerSMTPConfig::default_client_count_max(),
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WantsDelivery {
    #[serde(flatten)]
    pub(crate) parent: WantSMTP,
    #[serde(default)]
    pub(crate) smtp: InnerSMTPConfig,
}

impl ConfigBuilder<WantsDelivery> {
    pub fn with_delivery(
        self,
        spool_dir: impl Into<std::path::PathBuf>,
        queues: std::collections::HashMap<String, QueueConfig>,
    ) -> ConfigBuilder<WantsRules> {
        ConfigBuilder::<WantsRules> {
            state: WantsRules {
                parent: self.state,
                delivery: InnerDeliveryConfig {
                    spool_dir: spool_dir.into(),
                    queues,
                },
            },
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WantsRules {
    #[serde(flatten)]
    pub(crate) parent: WantsDelivery,
    pub(crate) delivery: InnerDeliveryConfig,
}

impl ConfigBuilder<WantsRules> {
    pub fn with_rules(
        self,
        source_dir: impl Into<std::path::PathBuf>,
        services: Vec<Service>,
    ) -> ConfigBuilder<WantsReplyCodes> {
        ConfigBuilder::<WantsReplyCodes> {
            state: WantsReplyCodes {
                parent: self.state,
                rules: InnerRulesConfig {
                    dir: source_dir.into(),
                    logs: InnerUserLogConfig::default(),
                    services,
                },
            },
        }
    }

    pub fn with_rules_and_logging(
        self,
        source_dir: impl Into<std::path::PathBuf>,
        services: Vec<Service>,
        log_file: impl Into<std::path::PathBuf>,
        log_level: log::LevelFilter,
        log_format: Option<String>,
    ) -> ConfigBuilder<WantsReplyCodes> {
        ConfigBuilder::<WantsReplyCodes> {
            state: WantsReplyCodes {
                parent: self.state,
                rules: InnerRulesConfig {
                    dir: source_dir.into(),
                    logs: InnerUserLogConfig {
                        file: log_file.into(),
                        level: log_level,
                        format: log_format,
                    },
                    services,
                },
            },
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WantsReplyCodes {
    #[serde(flatten)]
    pub(crate) parent: WantsRules,
    pub(crate) rules: InnerRulesConfig,
}

impl ConfigBuilder<WantsReplyCodes> {
    pub fn with_reply_codes(
        self,
        reply_codes: std::collections::HashMap<SMTPReplyCode, String>,
    ) -> ConfigBuilder<WantsBuild> {
        ConfigBuilder::<WantsBuild> {
            state: WantsBuild {
                parent: self.state,
                reply_codes: Codes { codes: reply_codes },
            },
        }
    }

    pub fn with_default_reply_codes(self) -> ConfigBuilder<WantsBuild> {
        self.with_reply_codes(Codes::default().codes)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WantsBuild {
    #[serde(flatten)]
    pub(crate) parent: WantsReplyCodes,
    #[serde(default)]
    pub(crate) reply_codes: Codes,
}

impl ConfigBuilder<WantsBuild> {
    pub fn build(mut self) -> anyhow::Result<ServerConfig> {
        let server_domain = &self
            .state
            .parent
            .parent
            .parent
            .parent
            .parent
            .parent
            .server
            .domain;
        let default_values = Codes::default();

        let mut reply_codes = self.state.reply_codes.codes;

        for i in <SMTPReplyCode as enum_iterator::IntoEnumIterator>::into_enum_iter() {
            reply_codes.insert(
                i,
                match reply_codes.get(&i) {
                    Some(v) => v,
                    None => default_values.get(&i),
                }
                .replace("{domain}", server_domain),
            );
        }
        self.state.reply_codes.codes = reply_codes;

        if let Some(smtps) = &mut self.state.parent.parent.parent.parent.smtps {
            smtps.fullchain = smtps
                .fullchain
                .to_str()
                .unwrap()
                .replace("{capath}", smtps.capath.to_str().unwrap())
                .replace("{domain}", server_domain)
                .into();
            smtps.private_key = smtps
                .private_key
                .to_str()
                .unwrap()
                .replace("{capath}", smtps.capath.to_str().unwrap())
                .replace("{domain}", server_domain)
                .into();

            if let Some(sni_maps) = &mut smtps.sni_maps {
                for i in sni_maps.iter_mut() {
                    *i = SniKey {
                        fullchain: i
                            .fullchain
                            .to_str()
                            .unwrap()
                            .replace("{domain}", &i.domain)
                            .replace("{capath}", smtps.capath.to_str().unwrap())
                            .into(),
                        private_key: i
                            .private_key
                            .to_str()
                            .unwrap()
                            .replace("{domain}", &i.domain)
                            .replace("{capath}", smtps.capath.to_str().unwrap())
                            .into(),
                        domain: i.domain.clone(),
                        protocol_version: i.protocol_version.clone(),
                    }
                }
            }
        };

        Ok(ServerConfig {
            version_requirement: self
                .state
                .parent
                .parent
                .parent
                .parent
                .parent
                .parent
                .parent
                .version_requirement,
            server: self.state.parent.parent.parent.parent.parent.parent.server,
            log: self.state.parent.parent.parent.parent.parent.logs,
            smtps: self.state.parent.parent.parent.parent.smtps,
            smtp: self.state.parent.parent.parent.smtp,
            delivery: self.state.parent.parent.delivery,
            rules: self.state.parent.rules,
            reply_codes: self.state.reply_codes,
        })
    }
}
