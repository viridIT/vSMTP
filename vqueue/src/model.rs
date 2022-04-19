use vsmtp_common::{
    collection,
    mail_context::MailContext,
    queue::Queue,
    re::{
        anyhow::{self, Context},
        serde_json,
    },
};

#[derive(Debug, Clone)]
pub struct QueueEntry {
    path: std::path::PathBuf,
    modified: std::time::SystemTime,
    pub(crate) message: MailContext,
}

impl TryFrom<std::path::PathBuf> for QueueEntry {
    type Error = anyhow::Error;

    fn try_from(path: std::path::PathBuf) -> Result<Self, Self::Error> {
        let value = std::fs::OpenOptions::new().read(true).open(&path)?;

        let metadata = value.metadata()?;
        let modified = metadata.modified()?;
        let message = std::fs::read_to_string(&path)
            .context(format!("Failed to read file: '{}'", path.display()))?;

        let message: MailContext = serde_json::from_str(&message)?;

        anyhow::Ok(Self {
            path,
            modified,
            message,
        })
    }
}

type MessageByLifetime = std::collections::HashMap<u64, Vec<std::path::PathBuf>>;

pub struct QueueContent {
    now: std::time::SystemTime,
    empty_token: char,
    dirpath: std::path::PathBuf,
    inner: std::collections::HashMap<String, MessageByLifetime>,
    queue: Queue,
}

impl QueueContent {
    pub fn add_entry(&mut self, key: &str, mut values: Vec<QueueEntry>) {
        let mut out = MessageByLifetime::new();

        for lifetime in Self::lifetimes() {
            let split_index = itertools::partition(&mut values, |i| {
                self.now
                    .duration_since(i.modified)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
                    / 60
                    < lifetime
            });
            let (to_push, new_values) = values.split_at(split_index);
            if !to_push.is_empty() {
                let to_push = to_push.iter().cloned().map(|i| i.path).collect::<Vec<_>>();

                out.entry(lifetime)
                    .and_modify(|v| v.extend(to_push.clone()))
                    .or_insert_with(|| to_push.clone());
            }
            values = new_values.to_vec();
        }
        out.insert(
            u64::MAX,
            values.into_iter().map(|i| i.path).collect::<Vec<_>>(),
        );

        assert!(!self.inner.contains_key(key));
        self.inner.insert(key.to_string(), out);
    }

    fn lifetimes() -> Vec<u64> {
        (0..9)
            .into_iter()
            .scan(5, |state, _| {
                let out = *state;
                *state *= 2;
                Some(out)
            })
            .collect()
    }
}

impl From<(Queue, std::path::PathBuf, char, std::time::SystemTime)> for QueueContent {
    fn from(
        (queue, dirpath, empty_token, now): (
            Queue,
            std::path::PathBuf,
            char,
            std::time::SystemTime,
        ),
    ) -> Self {
        Self {
            queue,
            empty_token,
            dirpath,
            now,
            inner: collection! {},
        }
    }
}

macro_rules! token_if_empty {
    ($t:expr, $e:expr) => {
        if $e != 0 {
            $e.to_string()
        } else {
            $t.to_string()
        }
    };
}

impl std::fmt::Display for QueueContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lifetimes = Self::lifetimes();

        f.write_fmt(format_args!(
            "{:<10} is at '{}' :",
            format!("{}", self.queue).to_uppercase(),
            self.dirpath.display()
        ))?;

        if self.inner.is_empty() {
            f.write_str(" <EMPTY>\n")?;
            return Ok(());
        }

        f.write_str("\n")?;

        f.write_fmt(format_args!("{:>15}", "T"))?;
        for i in &lifetimes {
            f.write_fmt(format_args!("{i:>5}"))?;
        }
        f.write_fmt(format_args!(
            "{max:>5}+",
            max = lifetimes.last().unwrap_or(&0)
        ))?;
        f.write_fmt(format_args!("\n"))?;

        f.write_fmt(format_args!(
            "{:>10}{:>5}",
            "TOTAL",
            token_if_empty!(
                self.empty_token,
                self.inner.iter().fold(0, |sum, (_, values)| values
                    .iter()
                    .fold(sum, |sum, (_, m)| { sum + m.len() }))
            )
        ))?;

        let sum_where = |lifetime: u64| {
            self.inner.iter().fold(0, |sum, (_, values)| {
                values
                    .iter()
                    .filter(|(l, _)| **l == lifetime)
                    .fold(sum, |sum, (_, m)| sum + m.len())
            })
        };

        for i in &lifetimes {
            f.write_fmt(format_args!(
                "{:>5}",
                token_if_empty!(self.empty_token, sum_where(*i))
            ))?;
        }
        f.write_fmt(format_args!(
            "{max:>5}",
            max = token_if_empty!(self.empty_token, sum_where(u64::MAX))
        ))?;
        f.write_fmt(format_args!("\n"))?;

        for (key, values) in &self.inner {
            f.write_fmt(format_args!(
                "{key:>10}{:>5}",
                token_if_empty!(
                    self.empty_token,
                    values.iter().fold(0, |sum, (_, m)| sum + m.len())
                )
            ))?;

            for i in &lifetimes {
                f.write_fmt(format_args!(
                    "{:>5}",
                    token_if_empty!(self.empty_token, values.get(i).map_or(0, Vec::len))
                ))?;
            }
            f.write_fmt(format_args!(
                "{max:>5}",
                max = token_if_empty!(self.empty_token, values.get(&u64::MAX).map_or(0, Vec::len))
            ))?;
            f.write_fmt(format_args!("\n"))?;
        }

        Ok(())
    }
}
