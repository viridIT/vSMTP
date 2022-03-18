#![allow(clippy::module_name_repetitions)]

use vsmtp_common::code::SMTPReplyCode;

use crate::config::{
    ConfigQueueDelivery, ConfigQueueWorking, ConfigServerSMTPError, ConfigServerSMTPTimeoutClient,
    ConfigServerTls, Service,
};

///
pub struct WantsVersion(pub(crate) ());

///
pub struct WantsServer {
    #[allow(dead_code)]
    pub(crate) parent: WantsVersion,
    pub(super) version_requirement: semver::VersionReq,
}

///
pub struct WantsServerSystem {
    pub(crate) parent: WantsServer,
    pub(super) domain: String,
    pub(super) client_count_max: i64,
}

///
pub struct WantsServerInterfaces {
    pub(crate) parent: WantsServerSystem,
    pub(super) user: String,
    pub(super) group: String,
    pub(super) thread_pool_receiver: usize,
    pub(super) thread_pool_processing: usize,
    pub(super) thread_pool_delivery: usize,
}

///
pub struct WantsServerLogs {
    pub(crate) parent: WantsServerInterfaces,
    pub(super) addr: Vec<std::net::SocketAddr>,
    pub(super) addr_submission: Vec<std::net::SocketAddr>,
    pub(super) addr_submissions: Vec<std::net::SocketAddr>,
}

///
pub struct WantsServerQueues {
    pub(crate) parent: WantsServerLogs,
    pub(super) filepath: std::path::PathBuf,
    pub(super) format: String,
    pub(super) level: std::collections::BTreeMap<String, log::LevelFilter>,
}

///
pub struct WantsServerTLSConfig {
    pub(crate) parent: WantsServerQueues,
    pub(super) dirpath: std::path::PathBuf,
    pub(super) working: ConfigQueueWorking,
    pub(super) delivery: ConfigQueueDelivery,
}

///
pub struct WantsServerSMTPConfig1 {
    pub(crate) parent: WantsServerTLSConfig,
    pub(super) tls: Option<ConfigServerTls>,
}

///
pub struct WantsServerSMTPConfig2 {
    pub(crate) parent: WantsServerSMTPConfig1,
    pub(super) rcpt_count_max: usize,
    pub(super) disable_ehlo: bool,
    pub(super) required_extension: Vec<String>,
}

///
pub struct WantsServerSMTPConfig3 {
    pub(crate) parent: WantsServerSMTPConfig2,
    pub(super) error: ConfigServerSMTPError,
    pub(super) timeout_client: ConfigServerSMTPTimeoutClient,
}

///
pub struct WantsApp {
    pub(crate) parent: WantsServerSMTPConfig3,
    pub(super) codes: std::collections::BTreeMap<SMTPReplyCode, String>,
}

///
pub struct WantsAppVSL {
    pub(crate) parent: WantsApp,
    pub(super) dirpath: std::path::PathBuf,
}

///
pub struct WantsAppLogs {
    pub(crate) parent: WantsAppVSL,
    pub(super) filepath: std::path::PathBuf,
}

///
pub struct WantsAppServices {
    pub(crate) parent: WantsAppLogs,
    pub(super) filepath: std::path::PathBuf,
    pub(super) level: log::LevelFilter,
    pub(super) format: String,
}

///
pub struct WantsValidate {
    pub(crate) parent: WantsAppServices,
    pub(super) services: std::collections::BTreeMap<String, Service>,
}
