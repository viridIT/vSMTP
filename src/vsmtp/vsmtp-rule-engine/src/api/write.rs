/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
use crate::api::{mail_context::message_id, Context, EngineResult, Message, Server, SharedObject};
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_common::re::serde_json;
use vsmtp_config::create_app_folder;

pub use write_rhai::*;

#[rhai::plugin::export_module]
mod write_rhai {

    /// write the current email to a specified folder.
    #[allow(clippy::needless_pass_by_value, clippy::module_name_repetitions)]
    #[rhai_fn(global, name = "write", return_raw, pure)]
    pub fn write_str(
        srv: &mut Server,
        mut ctx: Context,
        message: Message,
        dir: &str,
    ) -> EngineResult<()> {
        super::write(srv, &mut ctx, &message, dir)
    }

    /// write the current email to a specified folder.
    #[allow(clippy::needless_pass_by_value, clippy::module_name_repetitions)]
    #[rhai_fn(global, name = "write", return_raw, pure)]
    pub fn write_obj(
        srv: &mut Server,
        mut ctx: Context,
        message: Message,
        dir: SharedObject,
    ) -> EngineResult<()> {
        super::write(srv, &mut ctx, &message, &dir.to_string())
    }

    /// write the content of the current email with it's metadata in a json file.
    #[rhai_fn(global, name = "dump", return_raw, pure)]
    pub fn dump_str(srv: &mut Server, mut ctx: Context, dir: &str) -> EngineResult<()> {
        super::dump(srv, &mut ctx, dir)
    }

    /// write the content of the current email with it's metadata in a json file.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "dump", return_raw, pure)]
    pub fn dump_obj(srv: &mut Server, mut ctx: Context, dir: SharedObject) -> EngineResult<()> {
        super::dump(srv, &mut ctx, &dir.to_string())
    }
}

fn write(srv: &mut Server, ctx: &mut Context, message: &Message, dir: &str) -> EngineResult<()> {
    let mut dir =
        create_app_folder(&srv.config, Some(dir)).map_err::<Box<EvalAltResult>, _>(|err| {
            format!(
                "failed to write email at {}/{dir}: {err}",
                srv.config.app.dirpath.display()
            )
            .into()
        })?;
    dir.push(format!("{}.eml", message_id(ctx)?));

    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&dir)
        .map_err::<Box<EvalAltResult>, _>(|err| {
            format!("failed to write email at {dir:?}: {err}").into()
        })?;
    let mut writer = std::io::LineWriter::new(file);

    let body = &message
        .read()
        .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

    std::io::Write::write_all(&mut writer, body.inner().to_string().as_bytes())
        .map_err(|err| format!("failed to write email at {dir:?}: {err}").into())
}

fn dump(srv: &mut Server, ctx: &mut Context, dir: &str) -> EngineResult<()> {
    let mut dir =
        create_app_folder(&srv.config, Some(dir)).map_err::<Box<EvalAltResult>, _>(|err| {
            format!(
                "failed to dump email at {}/{dir}: {err}",
                srv.config.app.dirpath.display()
            )
            .into()
        })?;
    dir.push(format!("{}.json", message_id(ctx)?));

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&dir)
        .map_err::<Box<EvalAltResult>, _>(|err| {
            format!("failed to dump email at {dir:?}: {err}").into()
        })?;

    std::io::Write::write_all(
        &mut file,
        serde_json::to_string_pretty(
            &*ctx
                .read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?,
        )
        .map_err::<Box<EvalAltResult>, _>(|err| {
            format!("failed to dump email at {dir:?}: {err}").into()
        })?
        .as_bytes(),
    )
    .map_err(|err| format!("failed to dump email at {dir:?}: {err}").into())
}
