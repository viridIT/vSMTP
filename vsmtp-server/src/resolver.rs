use vsmtp_common::mail_context::MailContext;
use vsmtp_config::ServerConfig;

/// A trait allowing the [ServerVSMTP] to deliver a mail
#[async_trait::async_trait]
pub trait Resolver {
    /// the deliver method of the [Resolver] trait
    async fn deliver(&mut self, config: &ServerConfig, mail: &MailContext) -> anyhow::Result<()>;
}
