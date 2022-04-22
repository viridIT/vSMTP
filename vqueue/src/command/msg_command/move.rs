use vsmtp_common::{queue::Queue, re::anyhow};

use crate::command::get_message_path;

pub fn r#move(msg_id: &str, queue: Queue, queues_dirpath: &std::path::Path) -> anyhow::Result<()> {
    let message = get_message_path(msg_id, queues_dirpath)?;

    std::fs::rename(
        &message,
        vsmtp_common::queue_path!(
            queues_dirpath,
            queue,
            message
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Not a valid filename: '{}'", message.display()))?
        ),
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::r#move;
    use vsmtp_common::queue::Queue;

    #[test]
    fn basic() {
        let queues_dirpath = "./tmp/cmd_move";
        let msg_id = "toto";
        let filepath =
            vsmtp_common::queue_path!(create_if_missing => queues_dirpath, Queue::Working, msg_id)
                .unwrap();

        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filepath)
            .unwrap();

        vsmtp_common::queue_path!(create_if_missing => queues_dirpath, Queue::Dead).unwrap();
        r#move(
            msg_id,
            Queue::Dead,
            &std::path::PathBuf::from(queues_dirpath),
        )
        .unwrap();

        assert!(!filepath.exists());

        std::fs::remove_file(vsmtp_common::queue_path!(
            queues_dirpath,
            Queue::Dead,
            msg_id
        ))
        .unwrap();
    }
}
