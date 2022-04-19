use vsmtp_common::{queue::Queue, re::anyhow};

use crate::command::get_message_path;

pub fn r#move(msg_id: &str, queue: Queue, queues_dirpath: &std::path::Path) -> anyhow::Result<()> {
    let message = get_message_path(msg_id, queues_dirpath)?;

    std::fs::rename(
        &message,
        queue.to_path(queues_dirpath)?.join(
            message
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Not a valid filename: '{}'", message.display()))?,
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
        let filepath = Queue::Working.to_path(queues_dirpath).unwrap().join(msg_id);

        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filepath)
            .unwrap();

        r#move(
            msg_id,
            Queue::Dead,
            &std::path::PathBuf::from(queues_dirpath),
        )
        .unwrap();

        assert!(!filepath.exists());

        std::fs::remove_file(Queue::Dead.to_path(queues_dirpath).unwrap().join(msg_id)).unwrap();
    }
}
