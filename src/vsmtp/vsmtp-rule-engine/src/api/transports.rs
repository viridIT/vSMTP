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
use crate::api::{Context, EngineResult, SharedObject};
use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_common::transfer::ForwardTarget;

pub use transports_rhai::*;

#[allow(clippy::needless_pass_by_value)]
#[rhai::plugin::export_module]
mod transports_rhai {

    /// set the delivery method to "Forward" for a single recipient.
    #[rhai_fn(global, name = "forward", return_raw, pure)]
    pub fn forward_str_str(context: &mut Context, rcpt: &str, forward: &str) -> EngineResult<()> {
        let forward = <ForwardTarget as std::str::FromStr>::from_str(forward)
            .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?;

        set_transport_for(
            context,
            rcpt,
            &vsmtp_common::transfer::Transfer::Forward(forward),
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Forward" for a single recipient.
    #[rhai_fn(global, name = "forward", return_raw, pure)]
    pub fn forward_obj_str(
        context: &mut Context,
        rcpt: SharedObject,
        forward: &str,
    ) -> EngineResult<()> {
        let forward = <ForwardTarget as std::str::FromStr>::from_str(forward)
            .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?;

        set_transport_for(
            context,
            &rcpt.to_string(),
            &vsmtp_common::transfer::Transfer::Forward(forward),
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Forward" for a single recipient.
    #[rhai_fn(global, name = "forward", return_raw, pure)]
    pub fn forward_str_obj(
        context: &mut Context,
        rcpt: &str,
        forward: SharedObject,
    ) -> EngineResult<()> {
        let forward = <ForwardTarget as std::str::FromStr>::from_str(&forward.to_string())
            .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?;

        set_transport_for(
            context,
            rcpt,
            &vsmtp_common::transfer::Transfer::Forward(forward),
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Forward" for a single recipient.
    #[rhai_fn(global, name = "forward", return_raw, pure)]
    pub fn forward_obj_obj(
        context: &mut Context,
        rcpt: SharedObject,
        forward: SharedObject,
    ) -> EngineResult<()> {
        let forward = <ForwardTarget as std::str::FromStr>::from_str(&forward.to_string())
            .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?;

        set_transport_for(
            context,
            &rcpt.to_string(),
            &vsmtp_common::transfer::Transfer::Forward(forward),
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Forward" for all recipients.
    #[rhai_fn(global, name = "forward_all", return_raw, pure)]
    pub fn forward_all_str(context: &mut Context, forward: &str) -> EngineResult<()> {
        let forward = <ForwardTarget as std::str::FromStr>::from_str(forward)
            .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?;

        set_transport(context, &vsmtp_common::transfer::Transfer::Forward(forward))
    }

    ///
    #[rhai_fn(global, name = "forward_all", return_raw, pure)]
    pub fn forward_all_obj(context: &mut Context, forward: SharedObject) -> EngineResult<()> {
        let forward = <ForwardTarget as std::str::FromStr>::from_str(&forward.to_string())
            .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?;

        set_transport(context, &vsmtp_common::transfer::Transfer::Forward(forward))
    }

    /// set the delivery method to "Deliver" for a single recipient.
    #[rhai_fn(global, name = "deliver", return_raw, pure)]
    pub fn deliver_str(context: &mut Context, rcpt: &str) -> EngineResult<()> {
        set_transport_for(context, rcpt, &vsmtp_common::transfer::Transfer::Deliver)
            .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Deliver" for a single recipient.
    #[rhai_fn(global, name = "deliver", return_raw, pure)]
    pub fn deliver_obj(context: &mut Context, rcpt: SharedObject) -> EngineResult<()> {
        set_transport_for(
            context,
            &rcpt.to_string(),
            &vsmtp_common::transfer::Transfer::Deliver,
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Deliver" for all recipients.
    #[rhai_fn(global, return_raw, pure)]
    pub fn deliver_all(context: &mut Context) -> EngineResult<()> {
        set_transport(context, &vsmtp_common::transfer::Transfer::Deliver)
    }

    /// set the delivery method to "Mbox" for a single recipient.
    #[rhai_fn(global, name = "mbox", return_raw, pure)]
    pub fn mbox_str(context: &mut Context, rcpt: &str) -> EngineResult<()> {
        set_transport_for(context, rcpt, &vsmtp_common::transfer::Transfer::Mbox)
            .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Mbox" for a single recipient.
    #[rhai_fn(global, name = "mbox", return_raw, pure)]
    pub fn mbox_obj(context: &mut Context, rcpt: SharedObject) -> EngineResult<()> {
        set_transport_for(
            context,
            &rcpt.to_string(),
            &vsmtp_common::transfer::Transfer::Mbox,
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Mbox" for all recipients.
    #[rhai_fn(global, return_raw, pure)]
    pub fn mbox_all(context: &mut Context) -> EngineResult<()> {
        set_transport(context, &vsmtp_common::transfer::Transfer::Mbox)
    }

    /// set the delivery method to "Maildir" for a single recipient.
    #[rhai_fn(global, name = "maildir", return_raw, pure)]
    pub fn maildir_str(context: &mut Context, rcpt: &str) -> EngineResult<()> {
        set_transport_for(context, rcpt, &vsmtp_common::transfer::Transfer::Maildir)
            .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Maildir" for a single recipient.
    #[rhai_fn(global, name = "maildir", return_raw, pure)]
    pub fn maildir_obj(context: &mut Context, rcpt: SharedObject) -> EngineResult<()> {
        set_transport_for(
            context,
            &rcpt.to_string(),
            &vsmtp_common::transfer::Transfer::Maildir,
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Maildir" for all recipients.
    #[rhai_fn(global, return_raw, pure)]
    pub fn maildir_all(context: &mut Context) -> EngineResult<()> {
        set_transport(context, &vsmtp_common::transfer::Transfer::Maildir)
    }

    /// remove the delivery method for a specific recipient.
    #[rhai_fn(global, name = "disable_delivery", return_raw, pure)]
    pub fn disable_delivery_str(context: &mut Context, rcpt: &str) -> EngineResult<()> {
        set_transport_for(context, rcpt, &vsmtp_common::transfer::Transfer::None)
            .map_err(|err| err.to_string().into())
    }

    /// remove the delivery method for a specific recipient.
    #[rhai_fn(global, name = "disable_delivery", return_raw, pure)]
    pub fn disable_delivery_obj(context: &mut Context, rcpt: SharedObject) -> EngineResult<()> {
        set_transport_for(
            context,
            &rcpt.to_string(),
            &vsmtp_common::transfer::Transfer::None,
        )
        .map_err(|err| err.to_string().into())
    }

    /// remove the delivery method for all recipient.
    #[rhai_fn(global, return_raw, pure)]
    pub fn disable_delivery_all(context: &mut Context) -> EngineResult<()> {
        set_transport(context, &vsmtp_common::transfer::Transfer::None)
    }
}

/// set the transport method of a single recipient.
fn set_transport_for(
    context: &mut Context,
    search: &str,
    method: &vsmtp_common::transfer::Transfer,
) -> EngineResult<()> {
    context
        .write()
        .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?
        .envelop
        .rcpt
        .iter_mut()
        .find(|rcpt| rcpt.address.full() == search)
        .ok_or_else::<Box<EvalAltResult>, _>(|| format!("could not find rcpt '{}'", search).into())
        .map(|rcpt| rcpt.transfer_method = method.clone())
}

/// set the transport method of all recipients.
fn set_transport(
    context: &mut Context,
    method: &vsmtp_common::transfer::Transfer,
) -> EngineResult<()> {
    context
        .write()
        .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?
        .envelop
        .rcpt
        .iter_mut()
        .for_each(|rcpt| rcpt.transfer_method = method.clone());

    Ok(())
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::api::test::get_default_context;
    use vsmtp_common::{
        addr,
        rcpt::Rcpt,
        transfer::{ForwardTarget, Transfer},
    };

    #[test]
    fn test_set_transport_for() {
        let mut ctx = std::sync::Arc::new(std::sync::RwLock::new(get_default_context()));

        ctx.write()
            .unwrap()
            .envelop
            .rcpt
            .push(Rcpt::new(addr!("valid@rcpt.foo")));

        assert!(set_transport_for(&mut ctx, "valid@rcpt.foo", &Transfer::Deliver).is_ok());
        assert!(set_transport_for(&mut ctx, "invalid@rcpt.foo", &Transfer::Deliver).is_err());

        ctx.read()
            .unwrap()
            .envelop
            .rcpt
            .iter()
            .find(|rcpt| rcpt.address.full() == "valid@rcpt.foo")
            .map(|rcpt| {
                assert_eq!(rcpt.transfer_method, Transfer::Deliver);
            })
            .or_else(|| panic!("recipient transfer method is not valid"));
    }

    #[test]
    fn test_set_transport() {
        let mut ctx = std::sync::Arc::new(std::sync::RwLock::new(get_default_context()));

        set_transport(
            &mut ctx,
            &Transfer::Forward(ForwardTarget::Domain("mta.example.com".to_string())),
        )
        .unwrap();

        assert!(ctx
            .read()
            .unwrap()
            .envelop
            .rcpt
            .iter()
            .all(|rcpt| rcpt.transfer_method
                == Transfer::Forward(ForwardTarget::Domain("mta.example.com".to_string()))));

        set_transport(
            &mut ctx,
            &Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V4(
                "127.0.0.1".parse().unwrap(),
            ))),
        )
        .unwrap();

        assert!(ctx
            .read()
            .unwrap()
            .envelop
            .rcpt
            .iter()
            .all(|rcpt| rcpt.transfer_method
                == Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V4(
                    "127.0.0.1".parse().unwrap()
                )))));

        set_transport(
            &mut ctx,
            &Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V6(
                "::1".parse().unwrap(),
            ))),
        )
        .unwrap();

        assert!(ctx
            .read()
            .unwrap()
            .envelop
            .rcpt
            .iter()
            .all(|rcpt| rcpt.transfer_method
                == Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V6(
                    "::1".parse().unwrap()
                )))));
    }
}
