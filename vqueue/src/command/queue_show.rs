use vsmtp_common::{queue::Queue, re::anyhow};

use crate::{QueueContent, QueueEntry};

pub fn queue_show<OUT: std::io::Write>(
    queues: Vec<Queue>,
    queues_dirpath: &std::path::Path,
    empty_token: char,
    output: &mut OUT,
) -> anyhow::Result<()> {
    let now = std::time::SystemTime::now();

    for q in queues {
        let mut entries = q
            .list_entries(queues_dirpath)?
            .into_iter()
            .map(QueueEntry::try_from)
            .collect::<anyhow::Result<Vec<_>>>()?;
        entries.sort_by(|a, b| Ord::cmp(&a.message.envelop.helo, &b.message.envelop.helo));

        let mut content = QueueContent::from((q, q.to_path(&queues_dirpath)?, empty_token, now));

        for (key, values) in
            &itertools::Itertools::group_by(entries.into_iter(), |i| i.message.envelop.helo.clone())
        {
            content.add_entry(&key, values.into_iter().collect::<Vec<_>>());
        }

        output.write_fmt(format_args!("{content}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use vsmtp_common::{
        address::Address,
        envelop::Envelop,
        mail::{BodyType, Mail},
        mail_context::{Body, MailContext, MessageMetadata},
        queue::Queue,
        rcpt::Rcpt,
        re::strum,
        transfer::{EmailTransferStatus, Transfer},
    };

    use super::queue_show;

    #[test]
    fn working_and_delivery_empty() {
        let mut output = vec![];

        queue_show(
            vec![Queue::Working, Queue::Deliver],
            &std::path::PathBuf::from("./tmp/empty"),
            '.',
            &mut output,
        )
        .unwrap();

        pretty_assertions::assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            [
                "WORKING    is at './tmp/empty/working' : <EMPTY>\n",
                "DELIVER    is at './tmp/empty/deliver' : <EMPTY>\n",
            ]
            .concat(),
        );
    }

    #[test]
    fn all_empty() {
        let mut output = vec![];

        queue_show(
            <Queue as strum::IntoEnumIterator>::iter().collect::<Vec<_>>(),
            &std::path::PathBuf::from("./tmp/empty"),
            '.',
            &mut output,
        )
        .unwrap();

        pretty_assertions::assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            [
                "WORKING    is at './tmp/empty/working' : <EMPTY>\n",
                "DELIVER    is at './tmp/empty/deliver' : <EMPTY>\n",
                "DEFERRED   is at './tmp/empty/deferred' : <EMPTY>\n",
                "DEAD       is at './tmp/empty/dead' : <EMPTY>\n"
            ]
            .concat(),
        );
    }

    fn get_mail(msg_id: &str) -> MailContext {
        MailContext {
            connection_timestamp: std::time::SystemTime::now(),
            client_addr: "0.0.0.0:25".parse().unwrap(),
            envelop: Envelop {
                helo: "toto".to_string(),
                mail_from: Address::try_from("foo@domain.com".to_string()).unwrap(),
                rcpt: vec![Rcpt {
                    address: Address::try_from("foo+1@domain.com".to_string()).unwrap(),
                    transfer_method: Transfer::Mbox,
                    email_status: EmailTransferStatus::Waiting,
                }],
            },
            body: Body::Parsed(Box::new(Mail {
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
    fn dead_with_one() {
        let mut output = vec![];

        Queue::Dead
            .write_to_queue(
                &std::path::PathBuf::from("./tmp/dead_with_one"),
                &get_mail("foobar"),
            )
            .unwrap();

        queue_show(
            <Queue as strum::IntoEnumIterator>::iter().collect::<Vec<_>>(),
            &std::path::PathBuf::from("./tmp/dead_with_one"),
            '.',
            &mut output,
        )
        .unwrap();

        pretty_assertions::assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            [
                "WORKING    is at './tmp/dead_with_one/working' : <EMPTY>\n",
                "DELIVER    is at './tmp/dead_with_one/deliver' : <EMPTY>\n",
                "DEFERRED   is at './tmp/dead_with_one/deferred' : <EMPTY>\n",
                "DEAD       is at './tmp/dead_with_one/dead' :\n",
                "              T    5   10   20   40   80  160  320  640 1280 1280+\n",
                "     TOTAL    1    1    .    .    .    .    .    .    .    .    .\n",
                "      toto    1    1    .    .    .    .    .    .    .    .    .\n",
            ]
            .concat(),
        );
    }
}
