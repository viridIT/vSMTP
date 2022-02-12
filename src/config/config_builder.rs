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

use super::server_config::{
    Codes, DurationAlias, InnerDeliveryConfig, InnerLogConfig, InnerRulesConfig, InnerSMTPConfig,
    InnerSMTPErrorConfig, InnerServerConfig, InnerSmtpsConfig, ProtocolVersion,
    ProtocolVersionRequirement, QueueConfig, ServerConfig, SniKey, TlsSecurityLevel,
};

// #[derive(Clone)]
pub struct ConfigBuilder<State> {
    pub(crate) state: State,
}

impl ServerConfig {
    pub fn builder() -> ConfigBuilder<WantsServer> {
        ConfigBuilder {
            state: WantsServer(()),
        }
    }

    pub fn from_toml(data: &str) -> anyhow::Result<ServerConfig> {
        Ok(ConfigBuilder::<WantsBuild> {
            state: toml::from_str::<WantsBuild>(data)?,
        }
        .build())
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct WantsServer(pub(crate) ());

impl ConfigBuilder<WantsServer> {
    pub fn with_server(
        self,
        domain: impl Into<String>,
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
                },
            },
        }
    }

    pub fn with_rfc_port(
        self,
        domain: impl Into<String>,
        thread_count: Option<usize>,
    ) -> ConfigBuilder<WantsLogging> {
        self.with_server(
            domain,
            "0.0.0.0:25".parse().expect("valid address"),
            "0.0.0.0:587".parse().expect("valid address"),
            "0.0.0.0:465".parse().expect("valid address"),
            thread_count.unwrap_or_else(num_cpus::get),
        )
    }

    pub fn with_debug_port(
        self,
        domain: impl Into<String>,
        thread_count: Option<usize>,
    ) -> ConfigBuilder<WantsLogging> {
        self.with_server(
            domain,
            "0.0.0.0:10025".parse().expect("valid address"),
            "0.0.0.0:10587".parse().expect("valid address"),
            "0.0.0.0:10465".parse().expect("valid address"),
            thread_count.unwrap_or_else(num_cpus::get),
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct WantsLogging {
    #[serde(skip)]
    #[allow(unused)]
    pub(crate) parent: WantsServer,
    pub(crate) server: InnerServerConfig,
}

impl ConfigBuilder<WantsLogging> {
    pub fn with_logging(
        self,
        file: impl Into<String>,
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
            "./trash/log.log",
            crate::collection! {
                "default".to_string() => log::LevelFilter::Off
            },
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
// #[serde(deny_unknown_fields)]
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
        capath: impl Into<String>,
        preempt_cipherlist: bool,
        fullchain: impl Into<String>,
        private_key: impl Into<String>,
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
        fullchain: impl Into<String>,
        private_key: impl Into<String>,
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
// #[serde(deny_unknown_fields)]
pub struct WantSMTP {
    #[serde(flatten)]
    pub(crate) parent: WantSMTPS,
    pub(crate) smtps: Option<InnerSmtpsConfig>,
}

impl ConfigBuilder<WantSMTP> {
    pub fn with_smtp(
        self,
        disable_ehlo: bool,
        timeout_client: std::collections::HashMap<StateSMTP, std::time::Duration>,
        error_soft_count: i64,
        error_hard_count: i64,
        error_delay: std::time::Duration,
        rcpt_count_max: usize,
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
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
// #[serde(deny_unknown_fields)]
pub struct WantsDelivery {
    #[serde(flatten)]
    pub(crate) parent: WantSMTP,
    #[serde(default)]
    pub(crate) smtp: InnerSMTPConfig,
}

impl ConfigBuilder<WantsDelivery> {
    pub fn with_delivery(
        self,
        spool_dir: impl Into<String>,
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
// #[serde(deny_unknown_fields)]
pub struct WantsRules {
    #[serde(flatten)]
    pub(crate) parent: WantsDelivery,
    pub(crate) delivery: InnerDeliveryConfig,
}

impl ConfigBuilder<WantsRules> {
    pub fn with_rules(self, source_dir: impl Into<String>) -> ConfigBuilder<WantsReplyCodes> {
        ConfigBuilder::<WantsReplyCodes> {
            state: WantsReplyCodes {
                parent: self.state,
                rules: InnerRulesConfig {
                    dir: source_dir.into(),
                },
            },
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
// #[serde(deny_unknown_fields)]
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
// #[serde(deny_unknown_fields)]
pub struct WantsBuild {
    #[serde(flatten)]
    pub(crate) parent: WantsReplyCodes,
    #[serde(default)]
    pub(crate) reply_codes: Codes,
}

impl ConfigBuilder<WantsBuild> {
    pub fn build(mut self) -> ServerConfig {
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
                .replace("{capath}", &smtps.capath)
                .replace("{domain}", server_domain);
            smtps.private_key = smtps
                .private_key
                .replace("{capath}", &smtps.capath)
                .replace("{domain}", server_domain);

            if let Some(sni_maps) = &mut smtps.sni_maps {
                for i in sni_maps.iter_mut() {
                    *i = SniKey {
                        fullchain: i
                            .fullchain
                            .replace("{domain}", &i.domain)
                            .replace("{capath}", &smtps.capath),
                        private_key: i
                            .private_key
                            .replace("{domain}", &i.domain)
                            .replace("{capath}", &smtps.capath),
                        domain: i.domain.clone(),
                        protocol_version: i.protocol_version.clone(),
                    }
                }
            }
        };

        ServerConfig {
            server: self.state.parent.parent.parent.parent.parent.parent.server,
            log: self.state.parent.parent.parent.parent.parent.logs,
            smtps: self.state.parent.parent.parent.parent.smtps,
            smtp: self.state.parent.parent.parent.smtp,
            delivery: self.state.parent.parent.delivery,
            rules: self.state.parent.rules,
            reply_codes: self.state.reply_codes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init() -> anyhow::Result<()> {
        let _config = ServerConfig::builder()
            .with_rfc_port("test.server.com", None)
            .with_logging(
                "./tmp/log",
                std::collections::HashMap::<String, log::LevelFilter>::default(),
            )
            .with_safe_default_smtps(TlsSecurityLevel::May, "dummy", "dummy", None)
            .with_smtp(
                false,
                std::collections::HashMap::<StateSMTP, std::time::Duration>::default(),
                5,
                10,
                std::time::Duration::from_millis(100),
                1000,
            )
            .with_delivery(
                "/tmp/spool",
                std::collections::HashMap::<String, QueueConfig>::default(),
            )
            .with_rules("/tmp/re")
            .with_default_reply_codes()
            .build();

        Ok(())
    }

    #[test]
    fn init_no_smtps() -> anyhow::Result<()> {
        let _config = ServerConfig::builder()
            .with_rfc_port("test.server.com", None)
            .with_logging(
                "./tmp/log",
                std::collections::HashMap::<String, log::LevelFilter>::default(),
            )
            .without_smtps()
            .with_smtp(
                false,
                std::collections::HashMap::<StateSMTP, std::time::Duration>::default(),
                5,
                10,
                std::time::Duration::from_millis(100),
                1000,
            )
            .with_delivery(
                "/tmp/spool",
                std::collections::HashMap::<String, QueueConfig>::default(),
            )
            .with_rules("/tmp/re")
            .with_default_reply_codes()
            .build();

        // config.

        Ok(())
    }

    #[test]
    fn from_toml_template_simple() {
        assert_eq!(
            ServerConfig::from_toml(include_str!("template/simple.toml")).unwrap(),
            ServerConfig::builder()
                .with_rfc_port("testserver.com", None)
                .with_logging(
                    "/var/log/vsmtp/vsmtp.log",
                    crate::collection! {
                        "default".to_string() => log::LevelFilter::Warn
                    },
                )
                .without_smtps()
                .with_default_smtp()
                .with_delivery(
                    "/var/spool/vsmtp",
                    crate::collection! {
                        "working".to_string() => QueueConfig {
                            capacity: Some(32),
                            retry_max: None,
                            cron_period: None
                        },
                        "deliver".to_string() => QueueConfig {
                            capacity: Some(32),
                            retry_max: None,
                            cron_period: None
                        },
                        "deferred".to_string() => QueueConfig {
                            capacity: None,
                            retry_max: Some(10),
                            cron_period: Some(std::time::Duration::from_secs(10))
                        },
                    },
                )
                .with_rules("/etc/vsmtp/rules")
                .with_default_reply_codes()
                .build()
        );
    }

    #[test]
    fn from_toml_template_smtps() {
        assert_eq!(
            ServerConfig::from_toml(include_str!("template/smtps.toml")).unwrap(),
            ServerConfig::builder()
                .with_server(
                    "testserver.com",
                    "0.0.0.0:25".parse().expect("valid address"),
                    "0.0.0.0:587".parse().expect("valid address"),
                    "0.0.0.0:465".parse().expect("valid address"),
                    num_cpus::get(),
                )
                .with_logging(
                    "/var/log/vsmtp/vsmtp.log",
                    crate::collection! {
                        "default".to_string() => log::LevelFilter::Warn
                    },
                )
                .with_smtps(
                    TlsSecurityLevel::May,
                    ProtocolVersionRequirement(vec![ProtocolVersion(
                        rustls::ProtocolVersion::TLSv1_3
                    )]),
                    "./config/certs",
                    true,
                    "{capath}/certificate.crt",
                    "{capath}/privateKey.key",
                    std::time::Duration::from_millis(100),
                    Some(vec![SniKey {
                        domain: "testserver.com".to_string(),
                        private_key: "{capath}/rsa.{domain}.pem".to_string(),
                        fullchain: "{capath}/fullchain.{domain}.pem".to_string(),
                        protocol_version: None
                    }]),
                )
                .with_default_smtp()
                .with_delivery(
                    "./tmp/var/spool/vsmtp",
                    crate::collection! {
                        "working".to_string() => QueueConfig {
                            capacity: Some(32),
                            retry_max: None,
                            cron_period: None
                        },
                        "deliver".to_string() => QueueConfig {
                            capacity: Some(32),
                            retry_max: None,
                            cron_period: None
                        },
                        "deferred".to_string() => QueueConfig {
                            capacity: None,
                            retry_max: Some(10),
                            cron_period: Some(std::time::Duration::from_secs(10))
                        },
                    },
                )
                .with_rules("./config/rules")
                .with_default_reply_codes()
                .build()
        );
    }
}
