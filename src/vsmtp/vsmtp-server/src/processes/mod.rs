use vsmtp_common::{
    mail_context::{MailContext, MessageBody},
    re::{
        anyhow::{self, Context},
        serde_json,
    },
};

pub mod delivery;
pub mod postq;

pub async fn context_from_file_path(file: &std::path::Path) -> anyhow::Result<MailContext> {
    let content = tokio::fs::read_to_string(&file)
        .await
        .with_context(|| format!("Cannot read file '{}'", file.display()))?;

    serde_json::from_str::<MailContext>(&content)
        .with_context(|| format!("Cannot deserialize: '{content:?}'"))
}

pub async fn message_from_file_path(file: &std::path::Path) -> anyhow::Result<MessageBody> {
    let content = tokio::fs::read_to_string(&file)
        .await
        .with_context(|| format!("Cannot read file '{}'", file.display()))?;

    serde_json::from_str::<MessageBody>(&content)
        .with_context(|| format!("Cannot deserialize: '{content:?}'"))
}
