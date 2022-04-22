use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};

#[rhai::plugin::export_module]
pub mod rule_state {
    use crate::{
        modules::actions::transports::transports::disable_delivery_all,
        modules::actions::create_app_folder, modules::actions::MailContext,
        modules::mail_context::mail_context::message_id, modules::EngineResult,
        server_api::ServerAPI,
    };
    use vsmtp_common::status::Status;

    /// the transaction if forced accepted, skipping rules of next stages and going the pre-queue
    #[must_use]
    pub const fn faccept() -> Status {
        Status::Faccept
    }

    /// the transaction if accepted, skipping rules to the next stage
    #[must_use]
    pub const fn accept() -> Status {
        Status::Accept
    }

    /// the transaction continue to execute rule for that stage
    #[must_use]
    pub const fn next() -> Status {
        Status::Next
    }

    /// the transaction is denied, reply error to clients
    #[must_use]
    pub const fn deny() -> Status {
        Status::Deny
    }

    /// dump the current email into a quarantine queue, skipping delivery.
    /// the email is written in the specified app directory, inside the "queue" folder.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, return_raw)]
    pub fn quarantine(
        srv: &mut std::sync::Arc<ServerAPI>,
        mut ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        queue: &str,
    ) -> EngineResult<Status> {
        disable_delivery_all(&mut ctx)?;

        let mut path = create_app_folder(&srv.config, Some(queue))
            .map_err::<Box<EvalAltResult>, _>(|err| {
                format!(
                    "failed to dump email at {}/{queue}: {err}",
                    srv.config.app.dirpath.display()
                )
                .into()
            })?;

        path.push(format!("{}.json", message_id(&mut ctx)?));

        let ctx = ctx.read().map_err::<Box<EvalAltResult>, _>(|_| {
            "failed to quarantine email: mail context poisoned".into()
        })?;

        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            Ok(mut file) => {
                std::io::Write::write_all(
                    &mut file,
                    vsmtp_common::re::serde_json::to_string_pretty(&*ctx)
                        .map_err::<Box<EvalAltResult>, _>(|err| {
                            format!("failed to quarantine email: {err:?}").into()
                        })?
                        .as_bytes(),
                )
                .map_err::<Box<EvalAltResult>, _>(|err| {
                    format!("failed to quarantine email: {err:?}").into()
                })?;

                Ok(Status::Deny)
            }
            Err(err) => Err(format!("failed to quarantine email: {err:?}").into()),
        }
    }
}
