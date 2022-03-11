//! vSMTP executable

#![doc(html_no_source)]
#![deny(missing_docs)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::doc_markdown)]
#![allow(clippy::multiple_crate_versions)]

#[allow(clippy::module_name_repetitions)]
mod resolver {
    pub mod maildir_resolver;
    pub mod mbox_resolver;
    pub mod smtp_resolver;

    #[cfg(test)]
    use vsmtp_common::mail_context::MailContext;

    #[cfg(test)]
    pub fn get_default_context() -> MailContext {
        use vsmtp_common::{
            envelop::Envelop,
            mail_context::{Body, MessageMetadata},
        };

        MailContext {
            body: Body::Empty,
            connexion_timestamp: std::time::SystemTime::now(),
            client_addr: std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                0,
            ),
            envelop: Envelop::default(),
            metadata: Some(MessageMetadata {
                timestamp: std::time::SystemTime::now(),
                ..MessageMetadata::default()
            }),
        }
    }
}

pub use resolver::maildir_resolver::MailDirResolver;
pub use resolver::mbox_resolver::MBoxResolver;
pub use resolver::smtp_resolver::SMTPResolver;
