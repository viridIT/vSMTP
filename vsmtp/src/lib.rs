pub mod resolver {
    /// Protocol Maildir
    #[allow(clippy::module_name_repetitions)]
    pub mod maildir_resolver;

    /// Protocol Mailbox
    #[allow(clippy::module_name_repetitions)]
    pub mod mbox_resolver;

    /// Mail relaying
    #[allow(clippy::module_name_repetitions)]
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
