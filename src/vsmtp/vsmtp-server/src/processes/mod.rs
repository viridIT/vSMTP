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

/// Return a message body from a file path.
/// Try to parse the file as JSON, if it fails, try to parse it as plain text.
///
/// # Errors
///
/// * file(s) not found
/// * file found but failed to read
/// * file read but failed to serialize
pub async fn message_from_file_path(
    mut filepath: std::path::PathBuf,
) -> anyhow::Result<MessageBody> {
    filepath.set_extension("json");
    if filepath.exists() {
        let content = tokio::fs::read_to_string(&filepath)
            .await
            .with_context(|| format!("Cannot read file '{}'", filepath.display()))?;

        return serde_json::from_str::<MessageBody>(&content)
            .with_context(|| format!("Cannot deserialize: '{content:?}'"));
    }

    filepath.set_extension("eml");
    if filepath.exists() {
        let content = tokio::fs::read_to_string(&filepath)
            .await
            .with_context(|| format!("Cannot read file '{}'", filepath.display()))?;

        return Ok(MessageBody::Raw(
            content.lines().map(ToString::to_string).collect(),
        ));
    }
    anyhow::bail!("failed does not exist")
}
