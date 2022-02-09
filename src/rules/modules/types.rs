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
pub mod types {

    use crate::rules::{address::Address, modules::EngineResult, obj::Object};

    // shell service output (std::process::Output).

    #[rhai_fn(get = "stdout", return_raw)]
    pub fn stdout(this: &mut std::process::Output) -> EngineResult<String> {
        Ok(std::str::from_utf8(&this.stdout)
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .to_string())
    }

    #[rhai_fn(get = "stderr", return_raw)]
    pub fn stderr(this: &mut std::process::Output) -> EngineResult<String> {
        Ok(std::str::from_utf8(&this.stderr)
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .to_string())
    }

    #[rhai_fn(get = "code", return_raw)]
    pub fn code(this: &mut std::process::Output) -> EngineResult<i64> {
        Ok(this.status.code().ok_or_else::<Box<EvalAltResult>, _>(|| {
            "a SHELL process have been terminated by a signal".into()
        })? as i64)
    }

    // std::time::SystemTime

    #[rhai_fn(name = "to_string", return_raw)]
    pub fn time_to_string(this: &mut std::time::SystemTime) -> EngineResult<String> {
        Ok(format!(
            "{}",
            this.duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
                .as_secs()
        ))
    }

    #[rhai_fn(name = "to_debug", return_raw)]
    pub fn time_to_debug(this: &mut std::time::SystemTime) -> EngineResult<String> {
        Ok(format!(
            "{:?}",
            this.duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
        ))
    }

    // rules::address::Address

    #[rhai_fn(return_raw)]
    pub fn new_address(addr: &str) -> EngineResult<Address> {
        Address::new(addr).map_err(|error| error.to_string().into())
    }

    #[rhai_fn(name = "to_string")]
    pub fn address_to_string(this: &mut Address) -> String {
        this.full().to_string()
    }

    #[rhai_fn(name = "to_debug")]
    pub fn address_to_debug(this: &mut Address) -> String {
        format!("{this:?}")
    }

    #[rhai_fn(get = "full")]
    pub fn full(this: &mut Address) -> String {
        this.full().to_string()
    }

    #[rhai_fn(get = "local_part")]
    pub fn local_part(this: &mut Address) -> String {
        this.local_part().to_string()
    }

    #[rhai_fn(get = "domain")]
    pub fn domain(this: &mut Address) -> String {
        this.domain().to_string()
    }

    // vsmtp's rule engine obj syntax (std::sync::Arc<Object>).

    #[rhai_fn(name = "to_string")]
    pub fn object_to_string(this: &mut std::sync::Arc<Object>) -> String {
        this.to_string()
    }

    #[rhai_fn(name = "to_debug")]
    pub fn object_to_debug(this: &mut std::sync::Arc<Object>) -> String {
        format!("{:#?}", **this)
    }

    // rcpt container.
    pub type Rcpt = std::collections::HashSet<Address>;

    #[rhai_fn(get = "local_parts")]
    pub fn rcpt_local_parts(this: &mut Rcpt) -> Vec<String> {
        this.iter()
            .map(|addr| addr.local_part().to_string())
            .collect()
    }

    #[rhai_fn(get = "domains")]
    pub fn rcpt_domains(this: &mut Rcpt) -> Vec<String> {
        this.iter().map(|addr| addr.domain().to_string()).collect()
    }

    #[rhai_fn(name = "to_string")]
    pub fn rcpt_to_string(this: &mut Rcpt) -> String {
        format!("{this:?}")
    }

    #[rhai_fn(name = "to_debug")]
    pub fn rcpt_to_debug(this: &mut Rcpt) -> String {
        format!("{this:#?}")
    }

    #[rhai_fn(name = "to_string")]
    pub fn strings_to_string(this: &mut Vec<String>) -> String {
        format!("{this:?}")
    }

    #[rhai_fn(name = "to_debug")]
    pub fn strings_to_debug(this: &mut Vec<String>) -> String {
        format!("{this:#?}")
    }
}
