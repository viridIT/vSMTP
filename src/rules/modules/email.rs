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

    use crate::{rules::address::Address, smtp::mail::MailContext};
    use std::sync::{Arc, RwLock};

    #[rhai_fn(get = "client_addr", return_raw)]
    pub fn client_addr(
        this: &mut Arc<RwLock<MailContext>>,
    ) -> Result<std::net::SocketAddr, Box<EvalAltResult>> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .client_addr)
    }

    #[rhai_fn(get = "helo", return_raw)]
    pub fn helo(this: &mut Arc<RwLock<MailContext>>) -> Result<String, Box<EvalAltResult>> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .helo
            .clone())
    }

    #[rhai_fn(get = "mail_from", return_raw)]
    pub fn mail_from(this: &mut Arc<RwLock<MailContext>>) -> Result<Address, Box<EvalAltResult>> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .mail_from
            .clone())
    }

    #[rhai_fn(get = "rcpt", return_raw)]
    pub fn rcpt(
        this: &mut Arc<RwLock<MailContext>>,
    ) -> Result<std::collections::HashSet<Address>, Box<EvalAltResult>> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .clone())
    }

    #[rhai_fn(return_raw)]
    pub fn to_string(this: &mut Arc<RwLock<MailContext>>) -> Result<String, Box<EvalAltResult>> {
        Ok(format!(
            "{:?}",
            this.read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }

    #[rhai_fn(return_raw)]
    pub fn to_debug(this: &mut Arc<RwLock<MailContext>>) -> Result<String, Box<EvalAltResult>> {
        Ok(format!(
            "{:#?}",
            this.read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }
}
