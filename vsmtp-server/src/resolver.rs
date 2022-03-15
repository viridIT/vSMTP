/// A trait allowing the [ServerVSMTP] to deliver a mail
#[async_trait::async_trait]
pub trait Resolver {
    /// the deliver method of the [Resolver] trait
    async fn deliver(
        &mut self,
        config: &vsmtp_config::ServerConfig,
        mail: &vsmtp_common::mail_context::MailContext,
        rcpt: &vsmtp_common::rcpt::Rcpt,
    ) -> anyhow::Result<()>;
}

// /// A trait allowing the [ServerVSMTP] to deliver a mail
// #[async_trait::async_trait]
// pub trait Resolver {
//     /// the deliver method of the [Resolver] trait
//     async fn deliver(
//         &mut self,
//         config: &vsmtp_config::ServerConfig,
//         metadata: &vsmtp_common::mail_context::MessageMetadata,
//         envelop: &vsmtp_common::envelop::Envelop,
//         content: &str,
//     ) -> anyhow::Result<()>;
// }
