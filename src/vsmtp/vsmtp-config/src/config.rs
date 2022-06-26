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
#![allow(clippy::use_self)] // false positive

use serde_with::serde_as;
use vsmtp_common::{auth::Mechanism, re::log, CodeID, Reply};

/// This structure contains all the field to configure the server at the startup.
///
/// This structure will be loaded from a configuration file `-c, --config`
/// argument of the program. See [`crate::Config::from_toml`].
///
/// All field are optional and defaulted if missing.
///
/// You can also use the builder [`Config::builder`] to use the builder pattern,
/// and create an instance programmatically.
///
/// You can find examples of configuration files in
/// <https://github.com/viridIT/vSMTP/tree/develop/examples/config>
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// vSMTP's version requirement to parse this configuration file.
    #[serde(
        serialize_with = "crate::parser::semver::serialize",
        deserialize_with = "crate::parser::semver::deserialize"
    )]
    pub version_requirement: semver::VersionReq,
    /// see [`field::FieldServer`]
    #[serde(default)]
    pub server: field::FieldServer,
    /// see [`field::FieldApp`]
    #[serde(default)]
    pub app: field::FieldApp,
}

/// The inner field of the `vSMTP`'s configuration.
#[allow(clippy::module_name_repetitions)]
pub mod field {
    pub use super::*;

    /// This structure contains all the field to configure the server at the startup.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServer {
        /// Name of the server.
        ///
        /// Used with the response [`CodeID::Greetings`], and [`CodeID::Helo`],
        /// and [`CodeID::EhloPain`], and [`CodeID::EhloSecured`].
        // TODO: parse valid fqdn
        #[serde(default = "FieldServer::hostname")]
        pub domain: String,
        /// Maximum number of client served at the same time.
        ///
        /// The client will be rejected if the server is full.
        ///
        /// If this value is `-1`, then the server will accept any number of client.
        #[serde(default = "FieldServer::default_client_count_max")]
        pub client_count_max: i64,
        /// see [`FieldServerSystem`]
        #[serde(default)]
        pub system: FieldServerSystem,
        /// see [`FieldServerInterfaces`]
        #[serde(default)]
        pub interfaces: FieldServerInterfaces,
        /// see [`FieldServerLogs`]
        #[serde(default)]
        pub logs: FieldServerLogs,
        /// see [`FieldServerQueues`]
        #[serde(default)]
        pub queues: FieldServerQueues,
        /// see [`FieldServerTls`]
        pub tls: Option<FieldServerTls>,
        /// see [`FieldServerSMTP`]
        #[serde(default)]
        pub smtp: FieldServerSMTP,
        /// see [`FieldServerDNS`]
        #[serde(default)]
        pub dns: FieldServerDNS,
        /// see [`FieldServerVirtual`]
        #[serde(default)]
        pub r#virtual: std::collections::BTreeMap<String, FieldServerVirtual>,
    }

    /// The field related to the privileges used by `vSMTP`.
    #[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerSystem {
        /// User running the server after the drop of privileges using `setuid`.
        #[serde(default = "FieldServerSystem::default_user")]
        #[serde(
            serialize_with = "crate::parser::syst_user::serialize",
            deserialize_with = "crate::parser::syst_user::deserialize"
        )]
        pub user: users::User,
        /// Group running the server after the drop of privileges using `setgid`.
        #[serde(default = "FieldServerSystem::default_group")]
        #[serde(
            serialize_with = "crate::parser::syst_group::serialize",
            deserialize_with = "crate::parser::syst_group::deserialize"
        )]
        pub group: users::Group,
        /// Group right set for the local delivery (maildir/mbox).
        #[serde(default)]
        #[serde(
            serialize_with = "crate::parser::syst_group::opt_serialize",
            deserialize_with = "crate::parser::syst_group::opt_deserialize"
        )]
        pub group_local: Option<users::Group>,
        /// see [`FieldServerSystemThreadPool`]
        #[serde(default)]
        pub thread_pool: FieldServerSystemThreadPool,
    }

    impl PartialEq for FieldServerSystem {
        fn eq(&self, other: &Self) -> bool {
            self.user.uid() == other.user.uid()
                && self.group.gid() == other.group.gid()
                && self.group_local.as_ref().map(users::Group::gid)
                    == other.group_local.as_ref().map(users::Group::gid)
                && self.thread_pool == other.thread_pool
        }
    }

    impl Eq for FieldServerSystem {}

    /// The field related to the thread allocation.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerSystemThreadPool {
        /// Number of thread used by the pool `receiver`.
        ///
        /// This pool receive the client connection and handle the SMTP transaction.
        pub receiver: usize,
        /// Number of thread used by the pool `processing`.
        ///
        /// This pool forward the mails received by the `receiver` pool to the `delivery` pool.
        ///
        /// The mails treated has been accepted with a code [`CodeID::Ok`].
        ///
        /// "Offline" modification are applied here.
        pub processing: usize,
        /// Number of thread used by the pool `delivery`.
        ///
        /// This pool send the mails to the recipient, and handle the [`vsmtp_common::queue::Queue`]
        pub delivery: usize,
    }

    /// Address served by `vSMTP`. Either ipv4 or ipv6.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerInterfaces {
        /// List of address for the protocol SMTP. see [`vsmtp_common::ConnectionKind::Relay`]
        #[serde(default)]
        #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
        pub addr: Vec<std::net::SocketAddr>,
        /// List of address for the protocol ESMTPA. see [`vsmtp_common::ConnectionKind::Submission`]
        #[serde(default)]
        #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
        pub addr_submission: Vec<std::net::SocketAddr>,
        /// List of address for the protocol ESMTPSA. see [`vsmtp_common::ConnectionKind::Tunneled`]
        #[serde(default)]
        #[serde(deserialize_with = "crate::parser::socket_addr::deserialize")]
        pub addr_submissions: Vec<std::net::SocketAddr>,
    }

    /// The field related to the logs.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerLogs {
        /// Filepath of the server's log.
        ///
        /// Once the file as reached [`FieldServerLogs::size_limit`], an archive is created
        /// in the folder `{filepath}-ar`, named `trace.{N}.gz` (N being the number of archive).
        #[serde(default = "FieldServerLogs::default_filepath")]
        pub filepath: std::path::PathBuf,
        /// Format of the output, see [`log4rs::encode::pattern`]
        #[serde(default = "FieldServerLogs::default_format")]
        pub format: String,
        /// Customize the log level of the different part of the program.
        #[serde(default = "FieldServerLogs::default_level")]
        pub level: std::collections::BTreeMap<String, log::LevelFilter>,
        /// Size limit of the content in [`FieldServerLogs::filepath`] before archiving.
        #[serde(default = "FieldServerLogs::default_size_limit")]
        pub size_limit: u64,
        /// Number maximum of archive kept, the excess is deleted.
        #[serde(default = "FieldServerLogs::default_archive_count")]
        pub archive_count: u32,
    }

    /// The configuration of the [`vsmtp_common::queue::Queue::Working`]
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldQueueWorking {
        /// Size of the [`vsmtp_common::re::tokio::sync::mpsc::channel`]
        #[serde(default = "FieldQueueWorking::default_channel_size")]
        pub channel_size: usize,
    }

    /// The configuration of the [`vsmtp_common::queue::Queue::Deliver`] and the [`vsmtp_common::queue::Queue::Deferred`]
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldQueueDelivery {
        /// Size of the [`vsmtp_common::re::tokio::sync::mpsc::channel`]
        #[serde(default = "FieldQueueDelivery::default_channel_size")]
        pub channel_size: usize,
        /// Maximum number of attempt to deliver the mail before moving it to the [`vsmtp_common::queue::Queue::Dead`]
        #[serde(default = "FieldQueueDelivery::default_deferred_retry_max")]
        pub deferred_retry_max: usize,
        /// The mail in the  [`vsmtp_common::queue::Queue::Deferred`] are resend in a clock with this period.
        #[serde(with = "humantime_serde")]
        #[serde(default = "FieldQueueDelivery::default_deferred_retry_period")]
        pub deferred_retry_period: std::time::Duration,
    }

    /// The configuration of the filesystem for the [`vsmtp_common::queue::Queue`].
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerQueues {
        /// Path of the spool folder.
        ///
        /// The structure of the spool is the following:
        ///
        /// ```shell
        /// $> tree -L 2 /var/spool/vsmtp
        /// /var/spool/vsmtp
        /// ├── dead                   # mail failed to be delivered too many times
        /// ├── deferred               # mail failed to be delivered (1..N) times
        /// ├── deliver                # mail to be delivered
        /// ├── mails                  # the message body (received between DATA and "<CRLF>.<CRLF>"
        /// │   ├── <msg-id>.eml       # this file has not been modified (and stay in eml format)
        /// │   └── <msg-id-2>.json    # this file has been parsed, and stored in .json
        /// └── working                # mail to be processed
        ///
        pub dirpath: std::path::PathBuf,
        /// see [`FieldQueueWorking`]
        #[serde(default)]
        pub working: FieldQueueWorking,
        /// see [`FieldQueueDelivery`]
        #[serde(default)]
        pub delivery: FieldQueueDelivery,
    }

    /// The configuration of one virtual entry for the server.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerVirtual {
        /// see [`FieldServerVirtualTls`]
        pub tls: Option<FieldServerVirtualTls>,
        /// see [`FieldServerDNS`]
        pub dns: Option<FieldServerDNS>,
    }

    /// The TLS parameter for the **OUTGOING SIDE** of the virtual entry.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerVirtualTls {
        /// TLS protocol supported
        #[serde(
            serialize_with = "crate::parser::tls_protocol_version::serialize",
            deserialize_with = "crate::parser::tls_protocol_version::deserialize"
        )]
        pub protocol_version: Vec<rustls::ProtocolVersion>,
        /// Certificate chain to use for the TLS connection.
        pub certificate: TlsFile<rustls::Certificate>,
        /// Private key to use for the TLS connection.
        pub private_key: TlsFile<rustls::PrivateKey>,
        /// Policy of security for the TLS connection.
        #[serde(default = "FieldServerVirtualTls::default_sender_security_level")]
        pub sender_security_level: TlsSecurityLevel,
    }

    /// The policy of security for the TLS connection.
    #[derive(Debug, Copy, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    pub enum TlsSecurityLevel {
        /// Connection **MAY stay in plain text** for theirs transaction
        ///
        /// Connection **MAY UPGRADE** at any moment with a TLS tunnel (using STARTTLS mechanism)
        May,
        /// Connection **MUST BE UNDER TLS** tunnel (using STARTTLS mechanism or using port 465)
        Encrypt,
        /// DANE protocol using TLSA dns records to establish a secure connection with a distant server.
        Dane {
            /// port
            port: u16,
        },
    }

    #[doc(hidden)]
    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
    #[serde(transparent)]
    pub struct TlsFile<T> {
        #[serde(skip_serializing)]
        pub inner: T,
        pub path: std::path::PathBuf,
    }

    /// The TLS parameter for the **INCOMING SIDE** of the server (common with the virtual entry).
    // TODO: should use [`FieldServerVirtualTls`] flattened ?
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerTls {
        /// Policy of security for the TLS connection.
        pub security_level: TlsSecurityLevel,
        /// Ignore the client’s ciphersuite order.
        /// Instead, choose the top ciphersuite in the server list which is supported by the client.
        #[serde(default)]
        pub preempt_cipherlist: bool,
        /// Timeout for the TLS handshake. Sending a [`CodeID::Timeout`] to the client.
        #[serde(with = "humantime_serde")]
        #[serde(default = "FieldServerTls::default_handshake_timeout")]
        pub handshake_timeout: std::time::Duration,
        /// TLS protocol supported
        #[serde(
            serialize_with = "crate::parser::tls_protocol_version::serialize",
            deserialize_with = "crate::parser::tls_protocol_version::deserialize"
        )]
        pub protocol_version: Vec<rustls::ProtocolVersion>,
        #[serde(
            serialize_with = "crate::parser::tls_cipher_suite::serialize",
            deserialize_with = "crate::parser::tls_cipher_suite::deserialize",
            default = "FieldServerTls::default_cipher_suite"
        )]
        /// TLS cipher suite supported
        pub cipher_suite: Vec<rustls::CipherSuite>,
        /// Certificate chain to use for the TLS connection.
        pub certificate: TlsFile<rustls::Certificate>,
        /// Private key to use for the TLS connection.
        pub private_key: TlsFile<rustls::PrivateKey>,
    }

    /// Configuration of the client's error handling.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerSMTPError {
        /// The maximum number of errors before the client is delay between each response.
        ///
        /// `-1` to disable
        pub soft_count: i64,
        /// The maximum number of errors before the client is disconnected.
        ///
        /// `-1` to disable
        pub hard_count: i64,
        /// The delay used between each response, after `soft_count` errors.
        /// Unused if `soft_count` is `-1`.
        #[serde(with = "humantime_serde")]
        pub delay: std::time::Duration,
    }

    /// Configuration of the receiver timeout between each message.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerSMTPTimeoutClient {
        /// Delay between the connection and the first `HELO/EHLO`
        #[serde(with = "humantime_serde")]
        pub connect: std::time::Duration,
        /// Delay between the last `HELO/EHLO` and the next `MAIL FROM`
        #[serde(with = "humantime_serde")]
        pub helo: std::time::Duration,
        /// Delay between the last `MAIL FROM` and the next `RCPT TO`
        #[serde(with = "humantime_serde")]
        pub mail_from: std::time::Duration,
        /// Delay between the last `RCPT TO` and the next `DATA`
        #[serde(with = "humantime_serde")]
        pub rcpt_to: std::time::Duration,
        /// Delay between each message after the `DATA` command.
        #[serde(with = "humantime_serde")]
        pub data: std::time::Duration,
    }

    /// Policy of the extension AUTH.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerSMTPAuth {
        /// If `true`, the server will reject `MAIL FROM` commands if the client is not authenticated,
        /// sending [`CodeID::AuthRequired`]
        #[serde(default = "FieldServerSMTPAuth::default_must_be_authenticated")]
        pub must_be_authenticated: bool,
        /// Some mechanisms are considered unsecure under non-TLS connections.
        /// If `false`, the server will allow to use them even on clair connections.
        ///
        /// `false` by default.
        #[serde(default = "FieldServerSMTPAuth::default_enable_dangerous_mechanism_in_clair")]
        pub enable_dangerous_mechanism_in_clair: bool,
        /// List of mechanisms supported by the server.
        #[serde(default = "FieldServerSMTPAuth::default_mechanisms")]
        pub mechanisms: Vec<Mechanism>,
        /// If the AUTH exchange is canceled, the server will not consider the connection as closing,
        /// increasing the number of attempt failed, until `attempt_count_max`, producing an error.
        #[serde(default = "FieldServerSMTPAuth::default_attempt_count_max")]
        pub attempt_count_max: i64,
    }

    /// Parameters of the SMTP.
    #[serde_as]
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldServerSMTP {
        /// Maximum number of recipients received in the envelop, extra recipient will produce an [`CodeID::TooManyRecipients`].
        #[serde(default = "FieldServerSMTP::default_rcpt_count_max")]
        pub rcpt_count_max: usize,
        /// Disable the `EHLO` keywords, and thus the SMTP extension.
        ///
        /// default: `false`
        #[serde(default = "FieldServerSMTP::default_disable_ehlo")]
        pub disable_ehlo: bool,
        /// List of extension required to run the server, producing an error if not supported.
        ///
        /// UNUSED
        // TODO: parse extension enum
        #[serde(default = "FieldServerSMTP::default_required_extension")]
        pub required_extension: Vec<String>,
        /// SMTP's error policy.
        #[serde(default)]
        pub error: FieldServerSMTPError,
        /// SMTP's timeout policy.
        #[serde(default)]
        pub timeout_client: FieldServerSMTPTimeoutClient,
        /// Dictionary of the reply sent by the server during the SMTP transaction.
        #[serde(default)]
        #[serde_as(as = "std::collections::BTreeMap<serde_with::DisplayFromStr, _>")]
        pub codes: std::collections::BTreeMap<CodeID, Reply>,
        /// SMTP's authentication policy.
        pub auth: Option<FieldServerSMTPAuth>,
    }

    /// Configuration of the DNS resolver.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[allow(clippy::large_enum_variant)]
    #[serde(tag = "type", deny_unknown_fields)]
    pub enum FieldServerDNS {
        /// Using the resolver of the system (/etc/resolv.conf).
        #[serde(rename = "system")]
        System,
        /// Using the google DNS resolver.
        #[serde(rename = "google")]
        Google {
            /// Parameters
            #[serde(default)]
            options: ResolverOptsWrapper,
        },
        /// Using the google DNS resolver.
        #[serde(rename = "cloudflare")]
        CloudFlare {
            /// Parameters
            #[serde(default)]
            options: ResolverOptsWrapper,
        },
        /// A custom resolver.
        #[serde(rename = "custom")]
        Custom {
            /// Configuration of the resolver.
            config: trust_dns_resolver::config::ResolverConfig,
            /// Parameters
            #[serde(default)]
            options: ResolverOptsWrapper,
        },
    }

    /// Parameter for the DNS resolver.
    // TODO: remove that and use serde_with
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[allow(clippy::struct_excessive_bools)]
    pub struct ResolverOptsWrapper {
        /// Specify the timeout for a request. Defaults to 5 seconds
        #[serde(with = "humantime_serde")]
        #[serde(default = "ResolverOptsWrapper::default_timeout")]
        pub timeout: std::time::Duration,
        /// Number of retries after lookup failure before giving up. Defaults to 2
        #[serde(default = "ResolverOptsWrapper::default_attempts")]
        pub attempts: usize,
        /// Rotate through the resource records in the response (if there is more than one for a given name)
        #[serde(default = "ResolverOptsWrapper::default_rotate")]
        pub rotate: bool,
        /// Use DNSSec to validate the request
        #[serde(default = "ResolverOptsWrapper::default_dnssec")]
        pub dnssec: bool,
        /// The ip_strategy for the Resolver to use when lookup Ipv4 or Ipv6 addresses
        #[serde(default = "ResolverOptsWrapper::default_ip_strategy")]
        pub ip_strategy: trust_dns_resolver::config::LookupIpStrategy,
        /// Cache size is in number of records (some records can be large)
        #[serde(default = "ResolverOptsWrapper::default_cache_size")]
        pub cache_size: usize,
        /// Check /ect/hosts file before dns requery (only works for unix like OS)
        #[serde(default = "ResolverOptsWrapper::default_use_hosts_file")]
        pub use_hosts_file: bool,
        /// Number of concurrent requests per query
        ///
        /// Where more than one nameserver is configured, this configures the resolver to send queries
        /// to a number of servers in parallel. Defaults to 2; 0 or 1 will execute requests serially.
        #[serde(default = "ResolverOptsWrapper::default_num_concurrent_reqs")]
        pub num_concurrent_reqs: usize,
    }

    /// Configuration of the application run by `vSMTP`.
    #[derive(Default, Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldAppVSL {
        /// Entry point of the vsl application.
        pub filepath: Option<std::path::PathBuf>,
    }

    /// Application's parameter of the logs, same properties than [`FieldServerLogs`].
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldAppLogs {
        ///
        #[serde(default = "FieldAppLogs::default_filepath")]
        pub filepath: std::path::PathBuf,
        ///
        #[serde(default = "FieldAppLogs::default_level")]
        pub level: log::LevelFilter,
        ///
        #[serde(default = "FieldAppLogs::default_format")]
        pub format: String,
        ///
        #[serde(default = "FieldAppLogs::default_size_limit")]
        pub size_limit: u64,
        ///
        #[serde(default = "FieldAppLogs::default_archive_count")]
        pub archive_count: u32,
    }

    /// Configuration of the application run by `vSMTP`.
    #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct FieldApp {
        /// Folder under which artifact will be stored.
        #[serde(default = "FieldApp::default_dirpath")]
        pub dirpath: std::path::PathBuf,
        /// see [`FieldAppVSL`]
        #[serde(default)]
        pub vsl: FieldAppVSL,
        /// see [`FieldAppLogs`]
        #[serde(default)]
        pub logs: FieldAppLogs,
    }
}
