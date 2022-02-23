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
        rules::address::Address, rules::modules::types::Rcpt, rules::modules::EngineResult,
        smtp::mail::Body, smtp::mail::MailContext,
    };
    use std::io::Write;
    use std::sync::{Arc, RwLock};

    #[rhai_fn(global, get = "client_addr", return_raw)]
    pub fn client_addr(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<std::net::SocketAddr> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .client_addr)
    }

    #[rhai_fn(global, get = "helo", return_raw)]
    pub fn helo(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<String> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .helo
            .clone())
    }

    #[rhai_fn(global, get = "mail_from", return_raw)]
    pub fn mail_from(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<Address> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .mail_from
            .clone())
    }

    #[rhai_fn(global, return_raw)]
    pub fn rewrite_mail_from(
        this: &mut Arc<RwLock<MailContext>>,
        addr: String,
    ) -> EngineResult<()> {
        let addr = Address::new(&addr).map_err::<Box<EvalAltResult>, _>(|_| {
            format!(
                "could not rewrite mail_from with '{}' because it is not valid address",
                addr,
            )
            .into()
        })?;

        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .mail_from = addr;

        Ok(())
    }

    #[rhai_fn(global, get = "rcpt", return_raw)]
    pub fn rcpt(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<Rcpt> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .clone())
    }

    #[rhai_fn(global, return_raw)]
    pub fn rewrite_rcpt(
        this: &mut Arc<RwLock<MailContext>>,
        index: String,
        addr: String,
    ) -> EngineResult<()> {
        let index = Address::new(&index).map_err::<Box<EvalAltResult>, _>(|_| {
            format!(
                "could not rewrite address '{}' because it is not valid address",
                index,
            )
            .into()
        })?;

        let addr = Address::new(&addr).map_err::<Box<EvalAltResult>, _>(|_| {
            format!(
                "could not rewrite address '{}' with '{}' because it is not valid address",
                index, addr,
            )
            .into()
        })?;

        let rcpt = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt;

        rcpt.remove(&index);
        rcpt.insert(addr);

        Ok(())
    }

    #[rhai_fn(global, return_raw)]
    pub fn add_rcpt(this: &mut Arc<RwLock<MailContext>>, s: String) -> EngineResult<()> {
        let new_addr = Address::new(&s)
            .map_err(|_| format!("{} could not be converted to a valid rcpt address", s))?;

        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .insert(new_addr);

        Ok(())
    }

    #[rhai_fn(global, return_raw)]
    pub fn remove_rcpt(this: Arc<RwLock<MailContext>>, s: String) -> EngineResult<()> {
        let addr = Address::new(&s)
            .map_err(|_| format!("{} could not be converted to a valid rcpt address", s))?;

        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .remove(&addr);

        Ok(())
    }

    #[rhai_fn(global, get = "timestamp", return_raw)]
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

    #[rhai_fn(global, get = "message_id", return_raw)]
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

    #[rhai_fn(global, get = "retry", return_raw)]
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

    #[rhai_fn(global, return_raw)]
    pub fn to_string(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<String> {
        Ok(format!(
            "{:?}",
            this.read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }

    #[rhai_fn(global, return_raw)]
    pub fn to_debug(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<String> {
        Ok(format!(
            "{:#?}",
            this.read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }

    /// write the current email to a specified file.
    #[rhai_fn(global, return_raw)]
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
    #[rhai_fn(global, return_raw)]
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
    #[rhai_fn(global, return_raw)]
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

    #[rhai_fn(global, return_raw)]
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

    #[rhai_fn(global, return_raw)]
    pub fn disable_delivery(this: &mut Arc<RwLock<MailContext>>) -> EngineResult<()> {
        use_resolver(this, "none".to_string())
    }

    /// add a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw)]
    pub fn add_header(
        this: &mut Arc<RwLock<MailContext>>,
        header: &str,
        value: &str,
    ) -> EngineResult<()> {
        match &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .body
        {
            Body::Empty => {
                return Err(format!(
                    "failed to add header '{}': the body has not been received yet.",
                    header
                )
                .into())
            }
            Body::Raw(raw) => *raw = format!("{}: {}\n{}", header, value, raw),
            Body::Parsed(email) => email.headers.push((header.to_string(), value.to_string())),
        };

        Ok(())
    }

    //. push a recipient to the envelop, that will be invisible to all other recipients.
    #[rhai_fn(global, return_raw)]
    pub fn bcc(this: &mut Arc<RwLock<MailContext>>, bcc: &str) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .insert(
                Address::new(bcc).map_err(|_| {
                    format!("{} could not be converted to a valid rcpt address", bcc)
                })?,
            );

        Ok(())
    }
}
