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
use crate::api::{EngineResult, SharedObject};
use crate::dsl::service::{
    cmd::{run, CmdResult},
    Service,
};
use rhai::{
    plugin::{mem, FnAccess, FnNamespace, Module, PluginFunction, RhaiResult, TypeId},
    Dynamic, EvalAltResult, ImmutableString, NativeCallContext,
};

pub use services::*;

#[rhai::plugin::export_module]
mod services {

    ///
    #[rhai_fn(global, pure)]
    pub fn to_string(service: &mut std::sync::Arc<Service>) -> String {
        service.to_string()
    }

    ///
    #[rhai_fn(global, pure)]
    pub fn to_debug(service: &mut std::sync::Arc<Service>) -> String {
        format!("{service:#?}")
    }

    /// execute the given cmd service.
    #[rhai_fn(global, name = "cmd_run", return_raw, pure)]
    pub fn cmd_run(service: &mut std::sync::Arc<Service>) -> EngineResult<CmdResult> {
        if let Service::Cmd {
            timeout,
            user,
            group,
            command,
            args,
        } = &**service
        {
            run(timeout, command, user, group, args)
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())
        } else {
            Err("{service} cannot be run as a cmd script.".into())
        }
    }

    /// execute the given cmd service with dynamic arguments.
    #[rhai_fn(global, name = "cmd_run", return_raw, pure)]
    pub fn run_cmd_with_args(
        service: &mut std::sync::Arc<Service>,
        args: rhai::Array,
    ) -> EngineResult<CmdResult> {
        if let Service::Cmd {
            timeout,
            user,
            group,
            command,
            ..
        } = &**service
        {
            let args = args
                .into_iter()
                .map(rhai::Dynamic::try_cast)
                .collect::<Option<Vec<String>>>()
                .ok_or_else::<Box<EvalAltResult>, _>(|| {
                    "all cmd arguments must be strings".into()
                })?;
            run(timeout, command, user, group, &Some(args))
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())
        } else {
            Err(format!("{service} cannot be run as a cmd script.").into())
        }
    }

    /// a generic function to add a record to any database.
    #[rhai_fn(global, name = "db_add", return_raw, pure)]
    pub fn database_add(
        service: &mut std::sync::Arc<Service>,
        record: rhai::Array,
    ) -> EngineResult<()> {
        match &**service {
            Service::CSVDatabase {
                path,
                delimiter,
                fd,
                ..
            } => {
                let record = record
                    .into_iter()
                    .map(rhai::Dynamic::try_cast)
                    .collect::<Option<Vec<String>>>()
                    .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                        "all fields in a record must be strings".into()
                    })?;

                crate::dsl::service::databases::csv::add_record(path, *delimiter, fd, &record[..])
                    .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())
            }
            _ => Err("'db_add' can only be used on a database service.".into()),
        }
    }

    /// Remove a record from any database.
    #[rhai_fn(global, name = "db_rm", return_raw, pure)]
    pub fn database_remove_str(
        service: &mut std::sync::Arc<Service>,
        key: &str,
    ) -> EngineResult<()> {
        super::database_remove(service, key)
    }

    /// Remove a record from any database.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "db_rm", return_raw, pure)]
    pub fn database_remove_obj(
        service: &mut std::sync::Arc<Service>,
        key: SharedObject,
    ) -> EngineResult<()> {
        super::database_remove(service, &key.to_string())
    }

    /// a generic query by key implementation for all databases.
    #[rhai_fn(global, name = "db_query", return_raw, pure)]
    pub fn database_query_key_str(
        service: &mut std::sync::Arc<Service>,
        key: &str,
    ) -> EngineResult<rhai::Array> {
        super::database_query_key(service, key)
    }

    /// a generic query by key implementation for all databases.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "db_query", return_raw, pure)]
    pub fn database_query_key_obj(
        service: &mut std::sync::Arc<Service>,
        key: SharedObject,
    ) -> EngineResult<rhai::Array> {
        super::database_query_key(service, &key.to_string())
    }

    /// get the receiver address from a smtp service.
    #[rhai_fn(global, get = "receiver_address", return_raw, pure)]
    pub fn smtp_service_receiver_address(
        service: &mut std::sync::Arc<Service>,
    ) -> EngineResult<String> {
        match &**service {
            Service::Smtp { receiver, .. } => Ok(receiver.to_string()),
            _ => Err("only a smtp service has a receiver address".into()),
        }
    }
}

fn database_remove(service: &mut std::sync::Arc<Service>, key: &str) -> EngineResult<()> {
    match &**service {
        Service::CSVDatabase { path, .. } => {
            crate::dsl::service::databases::csv::remove_record(path, key)
                .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())
        }
        _ => Err("'db_add' can only be used on a database service.".into()),
    }
}

fn database_query_key(
    service: &mut std::sync::Arc<Service>,
    key: &str,
) -> EngineResult<rhai::Array> {
    match &**service {
        Service::CSVDatabase {
            path,
            delimiter,
            refresh,
            fd,
            ..
        } => crate::dsl::service::databases::csv::query_key(path, *delimiter, refresh, fd, key)
            .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?
            .map_or_else(
                || Ok(rhai::Array::default()),
                |record| {
                    Ok(record
                        .into_iter()
                        .map(|field| rhai::Dynamic::from(field.to_string()))
                        .collect())
                },
            ),
        _ => Err(format!("{service} cannot be run as a cmd script.").into()),
    }
}
