use crate::{
    config::server_config::ServerConfig, model::mail::MailContext, smtp::code::SMTPReplyCode,
};

pub mod maildir_resolver;

#[async_trait::async_trait]
pub trait DataEndResolver {
    async fn on_data_end(
        &mut self,
        config: &ServerConfig,
        mail: &MailContext,
    ) -> std::io::Result<SMTPReplyCode>;
}
