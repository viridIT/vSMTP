/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
use rhai::plugin::*;
#[allow(dead_code)]
#[export_module]
pub mod email {

    use crate::{
        config::log_channel::RULES, rules::address::Address, rules::modules::types::Rcpt,
        rules::modules::EngineResult, smtp::mail::Body, smtp::mail::MailContext,
    };
    use std::io::Write;
    use std::sync::{Arc, RwLock};

    #[rhai_fn(get = "client_addr", return_raw)]
    pub fn client_addr(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<std::net::SocketAddr> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .client_addr)
    }

    #[rhai_fn(get = "helo", return_raw)]
    pub fn helo(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<String> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .helo
            .clone())
    }

    #[rhai_fn(get = "mail_from", return_raw)]
    pub fn mail_from(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<Address> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .mail_from
            .clone())
    }

    #[rhai_fn(get = "rcpt", return_raw)]
    pub fn rcpt(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<Rcpt> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .clone())
    }

    #[rhai_fn(get = "timestamp", return_raw)]
    pub fn timestamp(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<std::time::SystemTime> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .metadata
            .as_ref()
            .ok_or_else::<Box<EvalAltResult>, _>(|| {
                "metadata are not available in this stage".into()
            })?
            .timestamp)
    }

    #[rhai_fn(get = "message_id", return_raw)]
    pub fn message_id(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<String> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .metadata
            .as_ref()
            .ok_or_else::<Box<EvalAltResult>, _>(|| {
                "metadata are not available in this stage".into()
            })?
            .message_id
            .clone())
    }

    #[rhai_fn(get = "retry", return_raw)]
    pub fn retry(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<u64> {
        this.read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .metadata
            .as_ref()
            .ok_or_else::<Box<EvalAltResult>, _>(|| {
                "metadata are not available in this stage".into()
            })?
            .retry
            .try_into()
            .map_err::<Box<EvalAltResult>, _>(|e: std::num::TryFromIntError| e.to_string().into())
    }

    // // exposed structure used to read & rewrite the incoming email's content.
    // .register_type::<Mail>()
    // .register_get("headers", |mail: &mut Mail| mail.headers.clone())
    // .register_get("body", |mail: &mut Mail| mail.body.clone())
    // .register_result_fn  ("rewrite_from", |mail: &mut Mail, value: &str| {
    //     if mail.body == BodyType::Undefined {
    //         Err("failed to execute 'RW_MAIL': body is undefined".into())
    //     } else {
    //         mail.rewrite_from(value);
    //         Ok(())
    //     }
    // })
    // .register_result_fn  ("rewrite_rcpt", |mail: &mut Mail, old: &str, new: &str| {
    //     if mail.body == BodyType::Undefined {
    //         Err("failed to execute 'RW_RCPT': body is undefined".into())
    //     } else {
    //         mail.rewrite_rcpt(old, new);
    //         Ok(())
    //     }
    // })
    // .register_result_fn  ("add_rcpt", |mail: &mut Mail, new: &str| {
    //     if mail.body == BodyType::Undefined {
    //         Err("failed to execute 'ADD_RCPT': body is undefined".into())
    //     } else {
    //         mail.add_rcpt(new);
    //         Ok(())
    //     }
    // })
    // .register_result_fn  ("delete_rcpt", |mail: &mut Mail, old: &str| {
    //     if mail.body == BodyType::Undefined {
    //         Err("failed to execute 'DEL_RCPT': body is undefined".into())
    //     } else {
    //         mail.delete_rcpt(old);
    //         Ok(())
    //     }
    // })

    #[rhai_fn(return_raw)]
    pub fn to_string(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<String> {
        Ok(format!(
            "{:?}",
            this.read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }

    #[rhai_fn(return_raw)]
    pub fn to_debug(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<String> {
        Ok(format!(
            "{:#?}",
            this.read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }

    /// checks if the object exists and check if it matches against the connect value.
    #[rhai_fn(return_raw)]
    pub fn is_connect(ctx: &mut Arc<RwLock<MailContext>>, ip: &str) -> EngineResult<bool> {
        match <std::net::IpAddr as std::str::FromStr>::from_str(ip) {
            Ok(ip) => Ok(ip
                == ctx
                    .read()
                    .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
                    .client_addr
                    .ip()),
            Err(_) => {
                log::error!(
                    target: RULES,
                    "tried to convert '{}' to an ip but conversion failed.",
                    ip
                );
                Ok(false)
            }
        }
    }

    /// write the current email to a specified file.
    #[rhai_fn(return_raw)]
    pub fn write(
        this: &mut Arc<RwLock<MailContext>>,
        path: &str,
    ) -> Result<(), Box<EvalAltResult>> {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            Ok(file) => {
                let mut writer = std::io::LineWriter::new(file);

                match &this
                    .read()
                    .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
                    .body
                {
                    Body::Empty => {
                        return Err(
                            "failed to write email: the body has not been received yet.".into()
                        )
                    }
                    Body::Raw(raw) => writer.write_all(raw.as_bytes()),
                    Body::Parsed(email) => {
                        let (body, headers) = email.to_raw();
                        writer.write_all(format!("{}\n{}", headers, body).as_bytes())
                    }
                }
            }
            .map_err(|err| format!("failed to write email: {err:?}").into()),
            Err(err) => Err(format!("failed to write email: {err:?}").into()),
        }
    }

    /// write the content of the current email in a json file.
    #[rhai_fn(return_raw)]
    pub fn dump(this: &mut Arc<RwLock<MailContext>>, path: &str) -> Result<(), Box<EvalAltResult>> {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            Ok(mut file) => file
                .write_all(
                    serde_json::to_string_pretty(&*this.read().map_err::<Box<EvalAltResult>, _>(
                        |err| format!("failed to dump email: {err:?}").into(),
                    )?)
                    .map_err::<Box<EvalAltResult>, _>(|err| {
                        format!("failed to dump email: {err:?}").into()
                    })?
                    .as_bytes(),
                )
                .map_err(|err| format!("failed to dump email: {err:?}").into()),
            Err(err) => Err(format!("failed to dump email: {err:?}").into()),
        }
    }

    // TODO: unfinished, queue parameter should point to a folder specified in toml config.
    /// dump the current email into a quarantine queue, skipping delivery.
    #[rhai_fn(return_raw)]
    pub fn quarantine(
        this: &mut Arc<RwLock<MailContext>>,
        queue: &str,
    ) -> Result<(), Box<EvalAltResult>> {
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(queue)
        {
            Ok(mut file) => {
                disable_delivery(this)?;

                file.write_all(
                    serde_json::to_string_pretty(&*this.write().map_err::<Box<EvalAltResult>, _>(
                        |err| format!("failed to dump email: {err:?}").into(),
                    )?)
                    .map_err::<Box<EvalAltResult>, _>(|err| {
                        format!("failed to dump email: {err:?}").into()
                    })?
                    .as_bytes(),
                )
                .map_err(|err| format!("failed to dump email: {err:?}").into())
            }
            Err(err) => Err(format!("failed to dump email: {err:?}").into()),
        }
    }

    #[rhai_fn(return_raw)]
    pub fn use_resolver(this: &mut Arc<RwLock<MailContext>>, resolver: String) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .metadata
            .as_mut()
            .ok_or_else::<Box<EvalAltResult>, _>(|| {
                "failed to use default resolver: email metadata are unavailable".into()
            })?
            .resolver = resolver;

        Ok(())
    }

    #[rhai_fn(return_raw)]
    pub fn disable_delivery(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<()> {
        use_resolver(this, "none".to_string())
    }
}
