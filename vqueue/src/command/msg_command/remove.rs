use crate::command::get_message_path;
use vsmtp_common::re::anyhow;

pub fn remove<OUT: std::io::Write, IN: std::io::BufRead>(
    msg_id: &str,
    confirmed: bool,
    queues_dirpath: &std::path::Path,
    output: &mut OUT,
    input: IN,
) -> anyhow::Result<()> {
    let message = get_message_path(msg_id, queues_dirpath)?;
    output.write_fmt(format_args!(
        "Removing file at location: '{}'\n",
        message.display()
    ))?;

    if !confirmed {
        output.write_all(b"Confirm ? [y|yes] ")?;
        output.flush()?;

        let confirmation = input
            .lines()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Fail to read line"))??;
        if !["y", "yes"].contains(&confirmation.to_lowercase().as_str()) {
            output.write_all(b"Canceled\n")?;
            return Ok(());
        }
    }

    std::fs::remove_file(&message)?;
    output.write_all(b"File removed\n")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::remove;
    use vsmtp_common::queue::Queue;

    #[test]
    fn confirmed() {
        let queues_dirpath = "./tmp/cmd_remove";
        let msg_id = "titi";
        let filepath = Queue::Working.to_path(queues_dirpath).unwrap().join(msg_id);

        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filepath)
            .unwrap();

        remove(
            msg_id,
            true,
            &std::path::PathBuf::from(queues_dirpath),
            &mut std::io::stdout(),
            std::io::stdin().lock(),
        )
        .unwrap();

        assert!(!filepath.exists());
    }

    #[test]
    fn not_confirmed() {
        let queues_dirpath = "./tmp/cmd_remove";
        let msg_id = "tata";
        let filepath = Queue::Working.to_path(queues_dirpath).unwrap().join(msg_id);

        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filepath)
            .unwrap();

        let mut output = vec![];

        remove(
            msg_id,
            false,
            &std::path::PathBuf::from(queues_dirpath),
            &mut output,
            b"yes\n" as &[u8],
        )
        .unwrap();

        assert!(!filepath.exists());
        assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            [
                "Removing file at location: './tmp/cmd_remove/working/tata'\n",
                "Confirm ? [y|yes] ",
                "File removed\n"
            ]
            .concat()
        );
    }

    #[test]
    fn canceled() {
        let queues_dirpath = "./tmp/cmd_remove";
        let msg_id = "tutu";
        let filepath = Queue::Working.to_path(queues_dirpath).unwrap().join(msg_id);

        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&filepath)
            .unwrap();

        let mut output = vec![];

        remove(
            msg_id,
            false,
            &std::path::PathBuf::from(queues_dirpath),
            &mut output,
            b"no\n" as &[u8],
        )
        .unwrap();

        assert!(filepath.exists());
        assert_eq!(
            std::str::from_utf8(&output).unwrap(),
            [
                "Removing file at location: './tmp/cmd_remove/working/tutu'\n",
                "Confirm ? [y|yes] ",
                "Canceled\n"
            ]
            .concat()
        );
    }
}
