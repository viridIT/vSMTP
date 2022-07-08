use crate::{command::get_message_path, MessageShowFormat};
use vsmtp_common::{
    mail_context::MailContext,
    re::{
        anyhow::{self, Context},
        serde_json,
    },
    MessageBody,
};

pub fn show<OUT: std::io::Write>(
    msg_id: &str,
    format: &MessageShowFormat,
    queues_dirpath: &std::path::Path,
    output: &mut OUT,
) -> anyhow::Result<()> {
    let mail_context: MailContext =
        serde_json::from_str(&get_message_path(msg_id, queues_dirpath).and_then(|path| {
            std::fs::read_to_string(&path)
                .context(format!("Failed to read file: '{}'", path.display()))
        })?)?;

    match format {
        MessageShowFormat::Eml => {
            let mut copy = std::path::PathBuf::from(queues_dirpath);
            copy.push(format!("mails/{msg_id}.eml"));

            match std::fs::read_to_string(copy).map(|s| MessageBody::try_from(s.as_str())) {
                Ok(Ok(message)) => output.write_all(message.inner().to_string().as_bytes()),
                Ok(Err(error)) => {
                    output.write_fmt(format_args!("Failed to deserialize message: '{error}'"))
                }
                Err(error) => output.write_fmt(format_args!("Failed to read message: '{error}'")),
            }
        }
        MessageShowFormat::Json => output.write_fmt(format_args!(
            "{}",
            serde_json::to_string_pretty(&mail_context)?
        )),
    }?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vsmtp_common::{
        addr,
        envelop::Envelop,
        mail_context::{ConnectionContext, MessageMetadata},
        queue::Queue,
        rcpt::Rcpt,
        transfer::{EmailTransferStatus, Transfer},
    };

    fn get_mail(msg_id: &str) -> (MailContext, MessageBody) {
        (
            MailContext {
                connection: ConnectionContext {
                    timestamp: std::time::SystemTime::now(),
                    credentials: None,
                    is_authenticated: false,
                    is_secured: false,
                    server_name: "testserver.com".to_string(),
                    server_address: "0.0.0.0:25".parse().unwrap(),
                },
                client_addr: "0.0.0.0:26".parse().unwrap(),
                envelop: Envelop {
                    helo: "toto".to_string(),
                    mail_from: addr!("foo@domain.com"),
                    rcpt: vec![Rcpt {
                        address: addr!("foo+1@domain.com"),
                        transfer_method: Transfer::Mbox,
                        email_status: EmailTransferStatus::Waiting {
                            timestamp: std::time::SystemTime::now(),
                        },
                    }],
                },
                metadata: Some(MessageMetadata {
                    timestamp: std::time::SystemTime::now(),
                    message_id: msg_id.to_string(),
                    skipped: None,
                }),
            },
            MessageBody::try_from(concat!(
                "From: foo2 foo <foo2@foo>\r\n",
                "Date: tue, 30 nov 2021 20:54:27 +0100\r\n",
                "\r\n",
                "Hello World!!\r\n",
            ))
            .unwrap(),
        )
    }

    #[test]
    fn eml() {
        let queues_dirpath = "./tmp/cmd_show";
        let msg_id = "titi";

        let (ctx, message) = get_mail(msg_id);

        Queue::Working
            .write_to_queue(&std::path::PathBuf::from(queues_dirpath), &ctx)
            .unwrap();

        message.write_to_mails(queues_dirpath, msg_id).unwrap();

        let mut output = vec![];

        show(
            msg_id,
            &MessageShowFormat::Eml,
            &std::path::PathBuf::from(queues_dirpath),
            &mut output,
        )
        .unwrap();

        pretty_assertions::assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            [
                "From: foo2 foo <foo2@foo>\r\n",
                "Date: tue, 30 nov 2021 20:54:27 +0100\r\n",
                "\r\n",
                "Hello World!!\r\n",
            ]
            .concat()
        );
    }

    #[test]
    fn json() {
        let queues_dirpath = "./tmp/cmd_show";
        let msg_id = "tutu";

        let (ctx, _) = get_mail(msg_id);

        Queue::Working
            .write_to_queue(&std::path::PathBuf::from(queues_dirpath), &ctx)
            .unwrap();

        let mut output = vec![];

        show(
            msg_id,
            &MessageShowFormat::Json,
            &std::path::PathBuf::from(queues_dirpath),
            &mut output,
        )
        .unwrap();

        pretty_assertions::assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            serde_json::to_string_pretty(&ctx).unwrap()
        );
    }
}
