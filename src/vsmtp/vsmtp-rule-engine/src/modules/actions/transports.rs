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
use vsmtp_common::{mail_context::MailContext, re::anyhow};

#[allow(clippy::needless_pass_by_value)]
#[rhai::plugin::export_module]
pub mod transports {
    use vsmtp_common::transfer::ForwardTarget;

    use crate::modules::types::types::Context;
    use crate::{dsl::object::Object, modules::EngineResult};

    #[rhai_fn(global, name = "forward", return_raw, pure)]
    pub fn forward_obj(
        this: &mut Context,
        rcpt: &str,
        forward: std::sync::Arc<Object>,
    ) -> EngineResult<()> {
        match &*forward {
            Object::Ip4(ip) => forward_str(this, rcpt, &ip.to_string()),
            Object::Ip6(ip) => forward_str(this, rcpt, &ip.to_string()),
            Object::Fqdn(fqdn) | Object::Str(fqdn) => forward_str(this, rcpt, fqdn),
            obj => Err(format!("{} is not a valid address to forward an email to.", obj).into()),
        }
    }

    /// set the delivery method to "Forward" for a single recipient.
    #[rhai_fn(global, name = "forward", return_raw, pure)]
    pub fn forward_str(this: &mut Context, rcpt: &str, forward: &str) -> EngineResult<()> {
        let forward = forward.parse::<std::net::SocketAddr>().map_or_else(
            |_| {
                forward.parse::<std::net::IpAddr>().map_or_else(
                    |_| ForwardTarget::Domain(forward.to_string()),
                    ForwardTarget::Ip,
                )
            },
            ForwardTarget::Socket,
        );

        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::Forward(forward),
        )
        .map_err(|err| err.to_string().into())
    }

    #[rhai_fn(global, name = "forward_all", return_raw, pure)]
    pub fn forward_all_obj(
        this: &mut Context,
        forward: std::sync::Arc<Object>,
    ) -> EngineResult<()> {
        match &*forward {
            Object::Ip4(ip) => forward_all_str(this, &ip.to_string()),
            Object::Ip6(ip) => forward_all_str(this, &ip.to_string()),
            Object::Fqdn(fqdn) | Object::Str(fqdn) => forward_all_str(this, fqdn),
            obj => Err(format!("{} is not a valid address to forward an email to.", obj).into()),
        }
    }

    /// set the delivery method to "Forward" for all recipients.
    #[rhai_fn(global, name = "forward_all", return_raw, pure)]
    pub fn forward_all_str(this: &mut Context, forward: &str) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Forward({
                forward
                    .parse::<std::net::IpAddr>()
                    .map_or(ForwardTarget::Domain(forward.to_string()), |ip| {
                        ForwardTarget::Ip(ip)
                    })
            }),
        );

        Ok(())
    }

    /// set the delivery method to "Deliver" for a single recipient.
    #[rhai_fn(global, return_raw, pure)]
    pub fn deliver(this: &mut Context, rcpt: &str) -> EngineResult<()> {
        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::Deliver,
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Deliver" for all recipients.
    #[rhai_fn(global, return_raw, pure)]
    pub fn deliver_all(this: &mut Context) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Deliver,
        );

        Ok(())
    }

    /// set the delivery method to "Mbox" for a single recipient.
    #[rhai_fn(global, return_raw, pure)]
    pub fn mbox(this: &mut Context, rcpt: &str) -> EngineResult<()> {
        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::Mbox,
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Mbox" for all recipients.
    #[rhai_fn(global, return_raw, pure)]
    pub fn mbox_all(this: &mut Context) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Mbox,
        );

        Ok(())
    }

    /// set the delivery method to "Maildir" for a single recipient.
    #[rhai_fn(global, return_raw, pure)]
    pub fn maildir(this: &mut Context, rcpt: &str) -> EngineResult<()> {
        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::Maildir,
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Maildir" for all recipients.
    #[rhai_fn(global, return_raw, pure)]
    pub fn maildir_all(this: &mut Context) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Maildir,
        );

        Ok(())
    }

    /// remove the delivery method for a specific recipient.
    #[rhai_fn(global, return_raw, pure)]
    pub fn disable_delivery(this: &mut Context, rcpt: &str) -> EngineResult<()> {
        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::None,
        )
        .map_err(|err| err.to_string().into())
    }

    /// remove the delivery method for all recipient.
    #[rhai_fn(global, return_raw, pure)]
    pub fn disable_delivery_all(this: &mut Context) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::None,
        );

        Ok(())
    }
}

/// set the transport method of a single recipient.
fn set_transport_for(
    ctx: &mut MailContext,
    search: &str,
    method: &vsmtp_common::transfer::Transfer,
) -> anyhow::Result<()> {
    ctx.envelop
        .rcpt
        .iter_mut()
        .find(|rcpt| rcpt.address.full() == search)
        .ok_or_else(|| anyhow::anyhow!("could not find rcpt '{}'", search))
        .map(|rcpt| rcpt.transfer_method = method.clone())
}

/// set the transport method of all recipients.
fn set_transport(ctx: &mut MailContext, method: &vsmtp_common::transfer::Transfer) {
    ctx.envelop
        .rcpt
        .iter_mut()
        .for_each(|rcpt| rcpt.transfer_method = method.clone());
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::modules::actions::test::get_default_context;
    use vsmtp_common::{
        addr,
        rcpt::Rcpt,
        transfer::{ForwardTarget, Transfer},
    };

    #[test]
    fn test_set_transport_for() {
        let mut ctx = get_default_context();

        ctx.envelop.rcpt.push(Rcpt::new(addr!("valid@rcpt.foo")));

        assert!(set_transport_for(&mut ctx, "valid@rcpt.foo", &Transfer::Deliver).is_ok());
        assert!(set_transport_for(&mut ctx, "invalid@rcpt.foo", &Transfer::Deliver).is_err());

        ctx.envelop
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
        let mut ctx = get_default_context();

        set_transport(
            &mut ctx,
            &Transfer::Forward(ForwardTarget::Domain("mta.example.com".to_string())),
        );

        assert!(ctx.envelop.rcpt.iter().all(|rcpt| rcpt.transfer_method
            == Transfer::Forward(ForwardTarget::Domain("mta.example.com".to_string()))));

        set_transport(
            &mut ctx,
            &Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V4(
                "127.0.0.1".parse().unwrap(),
            ))),
        );

        assert!(ctx.envelop.rcpt.iter().all(|rcpt| rcpt.transfer_method
            == Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V4(
                "127.0.0.1".parse().unwrap()
            )))));

        set_transport(
            &mut ctx,
            &Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V6(
                "::1".parse().unwrap(),
            ))),
        );

        assert!(ctx.envelop.rcpt.iter().all(|rcpt| rcpt.transfer_method
            == Transfer::Forward(ForwardTarget::Ip(std::net::IpAddr::V6(
                "::1".parse().unwrap()
            )))));
    }
}
