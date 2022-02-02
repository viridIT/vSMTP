use crate::smtp::{code::SMTPReplyCode, state::StateSMTP};

use super::server_config::{
    Codes, DurationAlias, InnerDeliveryConfig, InnerLogConfig, InnerRulesConfig, InnerSMTPConfig,
    InnerSMTPErrorConfig, InnerServerConfig, InnerTlsConfig, QueueConfig, ServerConfig,
    TlsSecurityLevel,
};

#[derive(Clone)]
pub struct ConfigBuilder<State> {
    pub(crate) state: State,
}

impl ServerConfig {
    pub fn builder() -> ConfigBuilder<WantsServer> {
        ConfigBuilder {
            state: WantsServer(()),
        }
    }
}

pub struct WantsServer(pub(crate) ());

impl ConfigBuilder<WantsServer> {
    pub fn with_server(
        self,
        domain: impl Into<String>,
        addr: std::net::SocketAddr,
        addr_submission: std::net::SocketAddr,
        addr_submissions: std::net::SocketAddr,
    ) -> ConfigBuilder<WantsLogging> {
        ConfigBuilder::<WantsLogging> {
            state: WantsLogging {
                parent: self.state,
                server: InnerServerConfig {
                    domain: domain.into(),
                    addr,
                    addr_submission,
                    addr_submissions,
                },
            },
        }
    }

    pub fn with_server_default_port(
        self,
        domain: impl Into<String>,
    ) -> ConfigBuilder<WantsLogging> {
        self.with_server(
            domain,
            "0.0.0.0:25".parse().expect("valid address"),
            "0.0.0.0:587".parse().expect("valid address"),
            "0.0.0.0:465".parse().expect("valid address"),
        )
    }
}

pub struct WantsLogging {
    #[allow(dead_code)]
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
}

pub struct WantSMTPS {
    pub(crate) parent: WantsLogging,
    pub(crate) logs: InnerLogConfig,
}

impl ConfigBuilder<WantSMTPS> {
    pub fn with_smtps(self) -> ConfigBuilder<WantSMTP> {
        ConfigBuilder::<WantSMTP> {
            state: WantSMTP {
                parent: self.state,
                smtps: Some(InnerTlsConfig {
                    security_level: TlsSecurityLevel::May,
                    protocol_version: todo!(),
                    capath: todo!(),
                    preempt_cipherlist: todo!(),
                    fullchain: todo!(),
                    private_key: todo!(),
                    handshake_timeout: todo!(),
                    sni_maps: todo!(),
                }),
            },
        }
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

pub struct WantSMTP {
    pub(crate) parent: WantSMTPS,
    pub(crate) smtps: Option<InnerTlsConfig>,
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
                    timeout_client: Some(
                        timeout_client
                            .into_iter()
                            .map(|(k, v)| (k, DurationAlias { alias: v }))
                            .collect(),
                    ),
                    error: InnerSMTPErrorConfig {
                        soft_count: error_soft_count,
                        hard_count: error_hard_count,
                        delay: error_delay,
                    },
                    rcpt_count_max: Some(rcpt_count_max),
                },
            },
        }
    }
}

pub struct WantsDelivery {
    pub(crate) parent: WantSMTP,
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

pub struct WantsRules {
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

pub struct WantsReplyCodes {
    pub(crate) parent: WantsRules,
    pub(crate) rules: InnerRulesConfig,
}

impl ConfigBuilder<WantsReplyCodes> {
    pub fn with_reply_codes(
        self,
        mut reply_codes: std::collections::HashMap<SMTPReplyCode, String>,
    ) -> ConfigBuilder<WantsBuild> {
        let server_domain = &self.state.parent.parent.parent.parent.parent.server.domain;
        let default_values = Codes::default();

        for i in <SMTPReplyCode as enum_iterator::IntoEnumIterator>::into_enum_iter() {
            reply_codes.insert(
                i.clone(),
                match reply_codes.get(&i) {
                    Some(v) => v,
                    None => default_values.get(&i),
                }
                .replace("{domain}", server_domain),
            );
        }

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

pub struct WantsBuild {
    pub(crate) parent: WantsReplyCodes,
    pub(crate) reply_codes: Codes,
}

impl ConfigBuilder<WantsBuild> {
    pub fn build(self) -> ServerConfig {
        ServerConfig {
            server: self.state.parent.parent.parent.parent.parent.parent.server,
            log: self.state.parent.parent.parent.parent.parent.logs,
            tls: self.state.parent.parent.parent.parent.smtps,
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
        let config = ServerConfig::builder()
            .with_server_default_port("test.server.com")
            .with_logging(
                "./tmp/log",
                std::collections::HashMap::<String, log::LevelFilter>::default(),
            )
            .with_smtps()
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
    fn init_no_smtps() -> anyhow::Result<()> {
        let config = ServerConfig::builder()
            .with_server_default_port("test.server.com")
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
}
