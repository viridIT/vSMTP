use serde_with::{serde_as, DisplayFromStr};

use crate::{
    mailprocessing::mail_receiver::StateSMTP, resolver::DataEndResolver, server::ServerVSMTP,
};

use super::custom_code::{CustomSMTPCode, SMTPCode};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerServerConfig {
    pub addr: std::net::SocketAddr,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerLogConfig {
    pub file: String,
    pub level: std::collections::HashMap<String, log::LevelFilter>,
}

#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum TlsSecurityLevel {
    None,
    May,
    Encrypt,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct SniKey {
    pub domain: String,
    pub private_key: String,
    pub fullchain: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerTlsConfig {
    pub security_level: TlsSecurityLevel,
    pub capath: Option<String>,
    pub preempt_cipherlist: bool,
    #[serde(with = "humantime_serde")]
    pub handshake_timeout: std::time::Duration,
    pub sni_maps: Option<Vec<SniKey>>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerSMTPErrorConfig {
    pub soft_count: i64,
    pub hard_count: i64,
    #[serde(with = "humantime_serde")]
    pub delay: std::time::Duration,
}

#[serde_as]
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerSMTPConfig {
    pub spool_dir: String,
    pub disable_ehlo: bool,
    #[serde_as(as = "std::collections::HashMap<DisplayFromStr, _>")]
    // #[serde(with = "humantime_serde")]
    pub timeout_client: std::collections::HashMap<StateSMTP, String>,
    pub error: InnerSMTPErrorConfig,
    // TODO: ? use serde_as ?
    pub code: Option<SMTPCode>,
}

impl InnerSMTPConfig {
    pub fn get_code(&self) -> &CustomSMTPCode {
        match self.code.as_ref() {
            None | Some(SMTPCode::Raw(_)) => {
                panic!("@get_code must be called after a valid conversion from to raw")
            }
            Some(SMTPCode::Serialized(code)) => code.as_ref(),
        }
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct InnerRulesConfig {
    pub dir: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ServerConfig {
    pub domain: String,
    pub server: InnerServerConfig,
    pub log: InnerLogConfig,
    pub tls: InnerTlsConfig,
    pub smtp: InnerSMTPConfig,
    pub rules: InnerRulesConfig,
}

impl ServerConfig {
    pub fn prepare(&mut self) -> &Self {
        self.prepare_inner(false)
    }

    pub fn prepare_default(&mut self) -> &Self {
        self.prepare_inner(true)
    }

    fn prepare_inner(&mut self, prepare_for_default: bool) -> &Self {
        self.smtp.code =
            Some(match &self.smtp.code {
                Some(SMTPCode::Raw(raw)) => SMTPCode::Serialized(Box::new(
                    CustomSMTPCode::from_raw(raw, self, prepare_for_default),
                )),
                None => SMTPCode::Serialized(Box::new(CustomSMTPCode::from_raw(
                    &std::collections::HashMap::<_, _>::new(),
                    self,
                    prepare_for_default,
                ))),
                Some(SMTPCode::Serialized(_)) => unreachable!(),
            });
        self
    }

    pub async fn build<R>(mut self) -> ServerVSMTP<R>
    where
        R: DataEndResolver + std::marker::Send,
    {
        self.prepare();
        ServerVSMTP::<R>::new(std::sync::Arc::new(self))
            .await
            .expect("Failed to create the server")
    }
}
