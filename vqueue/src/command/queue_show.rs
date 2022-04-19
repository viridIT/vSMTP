use vsmtp_common::{
    queue::Queue,
    re::anyhow::{self, Context},
};
use vsmtp_config::Config;

use crate::{QueueContent, QueueEntry};

fn queue_entries(
    queue: Queue,
    queues_dirpath: &std::path::Path,
) -> anyhow::Result<Vec<QueueEntry>> {
    let queue_path = queue.to_path(queues_dirpath)?;

    queue_path
        .read_dir()
        .context(format!("Error from read dir '{}'", queue_path.display()))?
        // TODO: raise error ?
        .filter_map(Result::ok)
        .map(QueueEntry::try_from)
        // TODO: ignore error ?
        .collect::<anyhow::Result<Vec<_>>>()
}

pub fn queue_show(queues: Vec<Queue>, config: &Config, empty_token: char) -> anyhow::Result<()> {
    let now = std::time::SystemTime::now();

    for q in queues {
        let mut entries = queue_entries(q, &config.server.queues.dirpath)?;
        entries.sort_by(|a, b| Ord::cmp(&a.message.envelop.helo, &b.message.envelop.helo));

        let mut content = QueueContent::from((
            q,
            q.to_path(&config.server.queues.dirpath).unwrap(),
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
