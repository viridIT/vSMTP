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
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};

#[rhai::plugin::export_module]
pub mod bcc {

    use crate::modules::types::types::Context;
    use crate::{dsl::object::Object, modules::EngineResult};
    use vsmtp_common::Address;

    /// add a recipient to the list recipient using a raw string.
    #[rhai_fn(global, name = "bcc", return_raw, pure)]
    pub fn from_str(this: &mut Context, bcc: &str) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .push(vsmtp_common::rcpt::Rcpt::new(
                Address::try_from(bcc.to_string()).map_err(|_| {
                    format!("'{}' could not be converted to a valid rcpt address", bcc)
                })?,
            ));

        Ok(())
    }

    /// add a recipient to the list recipient using an address.
    #[rhai_fn(global, name = "bcc", return_raw, pure)]
    pub fn from_addr(this: &mut Context, bcc: Address) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .push(vsmtp_common::rcpt::Rcpt::new(bcc));

        Ok(())
    }

    /// add a recipient to the list recipient using an object.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "bcc", return_raw, pure)]
    pub fn from_object(this: &mut Context, bcc: std::sync::Arc<Object>) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .envelop
            .rcpt
            .push(match &*bcc {
                Object::Address(addr) => vsmtp_common::rcpt::Rcpt::new(addr.clone()),
                Object::Str(string) => vsmtp_common::rcpt::Rcpt::new(
                    Address::try_from(string.clone()).map_err(|_| {
                        format!(
                            "'{}' could not be converted to a valid rcpt address",
                            string
                        )
                    })?,
                ),
                other => {
                    return Err(format!(
                        "'{}' could not be converted to a valid rcpt address",
                        other
                    )
                    .into())
                }
            });

        Ok(())
    }
}
