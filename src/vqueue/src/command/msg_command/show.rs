use crate::{command::get_message_path, MessageShowFormat};
use vsmtp_common::{
    mail_context::MailContext,
    re::{
        anyhow::{self, Context},
        serde_json,
    },
};

pub fn show<OUT: std::io::Write>(
    msg_id: &str,
    format: &MessageShowFormat,
    queues_dirpath: &std::path::Path,
    output: &mut OUT,
) -> anyhow::Result<()> {
    let message = get_message_path(msg_id, queues_dirpath).and_then(|path| {
        std::fs::read_to_string(&path).context(format!("Failed to read file: '{}'", path.display()))
    })?;

    let message: MailContext = serde_json::from_str(&message)?;

    match format {
        MessageShowFormat::Eml => output.write_fmt(format_args!("{}", message.body)),
        MessageShowFormat::Json => {
            output.write_fmt(format_args!("{}", serde_json::to_string_pretty(&message)?))
        }
    }?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vsmtp_common::{
        addr,
        envelop::Envelop,
        mail::{BodyType, Mail},
        mail_context::{ConnectionContext, MessageBody, MessageMetadata},
        queue::Queue,
        rcpt::Rcpt,
        transfer::{EmailTransferStatus, Transfer},
    };

    fn get_mail(msg_id: &str) -> MailContext {
        MailContext {
            connection: ConnectionContext {
                timestamp: std::time::SystemTime::now(),
                credentials: None,
                is_authenticated: false,
                is_secured: false,
                server_name: "testserver.com".to_string(),
            },
            client_addr: "0.0.0.0:25".parse().unwrap(),
            envelop: Envelop {
                helo: "toto".to_string(),
                mail_from: addr!("foo@domain.com"),
                rcpt: vec![Rcpt {
                    address: addr!("foo+1@domain.com"),
                    transfer_method: Transfer::Mbox,
                    email_status: EmailTransferStatus::Waiting,
                }],
            },
            body: MessageBody::Parsed(Box::new(Mail {
                headers: [
                    ("from", "foo2 foo <foo2@foo>"),
                    ("date", "tue, 30 nov 2021 20:54:27 +0100"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>(),
                body: BodyType::Regular(vec!["Hello World!!".to_string()]),
            })),
            metadata: Some(MessageMetadata {
                timestamp: std::time::SystemTime::now(),
                message_id: msg_id.to_string(),
                skipped: None,
            }),
        }
    }

    #[test]
    fn eml() {
        let queues_dirpath = "./tmp/cmd_show";
        let msg_id = "titi";

        Queue::Working
            .write_to_queue(&std::path::PathBuf::from(queues_dirpath), &get_mail(msg_id))
            .unwrap();

        let mut output = vec![];

        show(
            msg_id,
            &MessageShowFormat::Eml,
            &std::path::PathBuf::from(queues_dirpath),
            &mut output,
        )
        .unwrap();

        assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            [
                "from: foo2 foo <foo2@foo>\n",
                "date: tue, 30 nov 2021 20:54:27 +0100\n",
                "\n",
                "Hello World!!",
            ]
            .concat()
        );
    }

    #[test]
    fn json() {
        let queues_dirpath = "./tmp/cmd_show";
        let msg_id = "tutu";

        let mail = get_mail(msg_id);

        Queue::Working
            .write_to_queue(&std::path::PathBuf::from(queues_dirpath), &mail)
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
            serde_json::to_string_pretty(&mail).unwrap()
        );
    }
}
