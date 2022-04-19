use vsmtp_common::{queue::Queue, re::anyhow};
use vsmtp_config::Config;

use crate::{QueueContent, QueueEntry};

pub fn queue_show(queues: Vec<Queue>, config: &Config, empty_token: char) -> anyhow::Result<()> {
    let now = std::time::SystemTime::now();

    for q in queues {
        let mut entries = q
            .list_entries(&config.server.queues.dirpath)?
            .into_iter()
            .map(QueueEntry::try_from)
            .collect::<anyhow::Result<Vec<_>>>()?;
        entries.sort_by(|a, b| Ord::cmp(&a.message.envelop.helo, &b.message.envelop.helo));

        let mut content = QueueContent::from((
            q,
            q.to_path(&config.server.queues.dirpath)?,
            empty_token,
            now,
        ));

        for (key, values) in
            &itertools::Itertools::group_by(entries.into_iter(), |i| i.message.envelop.helo.clone())
        {
            content.add_entry(&key, values.into_iter().collect::<Vec<_>>());
        }

        println!("{content}");
    }
    Ok(())
}
