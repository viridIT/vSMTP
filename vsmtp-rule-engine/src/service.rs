use vsmtp_common::mail_context::{Body, MailContext};
use vsmtp_config::{log_channel::SRULES, service::Service};

/// Output generated by a service (shell)
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, Copy)]
pub struct ServiceResult {
    // TODO: do we want ? ExitStatus or Output ? see Child::wait_with_output
    status: std::process::ExitStatus,
}

impl ServiceResult {
    pub const fn new(status: std::process::ExitStatus) -> Self {
        Self { status }
    }

    pub fn has_code(self) -> bool {
        self.get_code().is_some()
    }

    pub fn get_code(self) -> Option<i64> {
        self.status.code().map(i64::from)
    }

    pub fn has_signal(self) -> bool {
        self.get_signal().is_some()
    }

    pub fn get_signal(self) -> Option<i64> {
        std::os::unix::prelude::ExitStatusExt::signal(&self.status).map(i64::from)
    }
}

impl std::fmt::Display for ServiceResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.status))
    }
}

/// run the service using an email context.
/// # Errors
///
/// * if the body of the email is empty.
/// * if the user used to launch commands is not found.
/// * if the group used to launch commands is not found.
/// * if the shell service failed to spawn.
/// * if the shell returned an error.
pub fn run(this: &Service, ctx: &MailContext) -> anyhow::Result<ServiceResult> {
    let body = match &ctx.body {
        Body::Empty => anyhow::bail!("could not run service: body of the email is empty",),
        Body::Raw(raw) => raw.clone(),
        Body::Parsed(parsed) => parsed.to_raw(),
    };

    match this {
        Service::UnixShell {
            timeout,
            command,
            args,
            user,
            group,
            ..
        } => {
            let mut child = std::process::Command::new(command);
            if let Some(args) = args {
                for i in args.split_whitespace() {
                    child.arg(i.replace("{mail}", &body));
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

            log::trace!(target: SRULES, "running command: {:#?}", child);

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
