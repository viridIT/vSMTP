use crate::{
    config::{log_channel::RULES, server_config::Service},
    smtp::mail::{Body, MailContext},
};

#[derive(Debug, Clone, Copy)]
pub struct ServiceResult {
    // TODO: do we want ? ExitStatus or Output ? see Child::wait_with_output
    status: std::process::ExitStatus,
}

impl ServiceResult {
    pub fn new(status: std::process::ExitStatus) -> Self {
        Self { status }
    }

    pub fn has_code(&self) -> bool {
        self.get_code().is_some()
    }

    pub fn get_code(&self) -> Option<i64> {
        self.status.code().map(|i| i as i64)
    }

    pub fn has_signal(&self) -> bool {
        self.get_signal().is_some()
    }

    pub fn get_signal(&self) -> Option<i64> {
        std::os::unix::prelude::ExitStatusExt::signal(&self.status).map(|i| i as i64)
    }
}

impl std::fmt::Display for ServiceResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.status))
    }
}

impl Service {
    pub fn run(
        &self,
        ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
    ) -> anyhow::Result<ServiceResult> {
        match self {
            Service::UnixShell {
                timeout,
                command,
                args,
                user,
                group,
                ..
            } => {
                // TODO: CommandExt / uid/gid

                let mut child = std::process::Command::new(command);
                if let Some(args) = args {
                    let guard = ctx.read().expect("mutex is poisoned");
                    for i in args.split_whitespace() {
                        child.arg(i.replace(
                            "{mail}",
                            match &guard.body {
                                Body::Empty => todo!(),
                                Body::Raw(raw) => raw,
                                Body::Parsed(_) => todo!(),
                            },
                        ));
                    }
                }

                if let Some(user_name) = user {
                    if let Some(user) = users::get_user_by_name(&user_name) {
                        std::os::unix::prelude::CommandExt::uid(&mut child, user.uid());
                    } else {
                        anyhow::bail!("UnixShell user not found: '{user_name}'")
                    }
                }
                if let Some(group_name) = group {
                    if let Some(group) = users::get_group_by_name(group_name) {
                        std::os::unix::prelude::CommandExt::gid(&mut child, group.gid());
                    } else {
                        anyhow::bail!("UnixShell group not found: '{group_name}'")
                    }
                }

                log::trace!(target: RULES, "running command: {:#?}", child);

                let mut child = match child.spawn() {
                    Ok(child) => child,
                    Err(err) => anyhow::bail!("UnixShell process failed to spawn: {err:?}"),
                };

                let status = match wait_timeout::ChildExt::wait_timeout(&mut child, *timeout) {
                    Ok(status) => status.unwrap_or_else(|| {
                        child.kill().expect("child has already exited");
                        child.wait().expect("command wasn't running")
                    }),

                    Err(err) => anyhow::bail!("UnixShell unexpected error: {err:?}"),
                };

                Ok(ServiceResult::new(status))
            }
        }
    }
}
