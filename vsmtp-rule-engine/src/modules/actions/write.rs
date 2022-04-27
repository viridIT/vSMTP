use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};

#[rhai::plugin::export_module]
pub mod write {

    use crate::{
        modules::actions::create_app_folder, modules::actions::MailContext,
        modules::mail_context::mail_context::message_id, modules::EngineResult,
        server_api::ServerAPI,
    };
    use vsmtp_common::mail_context::Body;

    /// write the current email to a specified folder.
    #[rhai_fn(global, return_raw)]
    pub fn write(
        srv: &mut std::sync::Arc<ServerAPI>,
        mut ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        dir: &str,
    ) -> EngineResult<()> {
        let mut dir =
            create_app_folder(&srv.config, Some(dir)).map_err::<Box<EvalAltResult>, _>(|err| {
                format!(
                    "failed to write email at {}/{dir}: {err}",
                    srv.config.app.dirpath.display()
                )
                .into()
            })?;

        dir.push(format!("{}.eml", message_id(&mut ctx)?));

        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&dir)
        {
            Ok(file) => {
                let mut writer = std::io::LineWriter::new(file);

                match &ctx
                    .read()
                    .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
                    .body
                {
                    Body::Empty => {
                        return Err(
                            "failed to write email: the body has not been received yet.".into()
                        )
                    }
                    Body::Raw(raw) => std::io::Write::write_all(&mut writer, raw.as_bytes()),
                    Body::Parsed(email) => {
                        std::io::Write::write_all(&mut writer, email.to_raw().as_bytes())
                    }
                }
            }
            .map_err(|err| format!("failed to write email at {dir:?}: {err}").into()),
            Err(err) => Err(format!("failed to write email at {dir:?}: {err}").into()),
        }
    }

    /// write the content of the current email with it's metadata in a json file.
    #[rhai_fn(global, return_raw)]
    pub fn dump(
        srv: &mut std::sync::Arc<ServerAPI>,
        mut ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        dir: &str,
    ) -> EngineResult<()> {
        let mut dir =
            create_app_folder(&srv.config, Some(dir)).map_err::<Box<EvalAltResult>, _>(|err| {
                format!(
                    "failed to dump email at {}/{dir}: {err}",
                    srv.config.app.dirpath.display()
                )
                .into()
            })?;

        dir.push(format!("{}.json", message_id(&mut ctx)?));

        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&dir)
        {
            Ok(mut file) => std::io::Write::write_all(
                &mut file,
                vsmtp_common::re::serde_json::to_string_pretty(
                    &*ctx
                        .read()
                        .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?,
                )
                .map_err::<Box<EvalAltResult>, _>(|err| {
                    format!("failed to dump email at {dir:?}: {err}").into()
                })?
                .as_bytes(),
            )
            .map_err(|err| format!("failed to dump email at {dir:?}: {err}").into()),
            Err(err) => Err(format!("failed to dump email at {dir:?}: {err}").into()),
        }
    }
}
