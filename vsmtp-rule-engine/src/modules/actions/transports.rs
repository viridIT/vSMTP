use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};
use vsmtp_common::{mail_context::MailContext, re::anyhow};

#[rhai::plugin::export_module]
pub mod transports {
    use vsmtp_common::transfer::ForwardTarget;

    use crate::{modules::actions::MailContext, modules::EngineResult};

    /// set the delivery method to "Forward" for a single recipient.
    #[rhai_fn(global, return_raw)]
    pub fn forward(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        rcpt: &str,
        forward: &str,
    ) -> EngineResult<()> {
        set_transport_for(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            rcpt,
            &vsmtp_common::transfer::Transfer::Forward({
                forward
                    .parse::<std::net::IpAddr>()
                    .map_or(ForwardTarget::Domain(forward.to_string()), |ip| {
                        ForwardTarget::Ip(ip)
                    })
            }),
        )
        .map_err(|err| err.to_string().into())
    }

    /// set the delivery method to "Forward" for all recipients.
    #[rhai_fn(global, return_raw)]
    pub fn forward_all(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        forward: &str,
    ) -> EngineResult<()> {
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
    #[rhai_fn(global, return_raw)]
    pub fn deliver(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        rcpt: &str,
    ) -> EngineResult<()> {
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
    #[rhai_fn(global, return_raw)]
    pub fn deliver_all(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
    ) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Deliver,
        );

        Ok(())
    }

    /// set the delivery method to "Mbox" for a single recipient.
    #[rhai_fn(global, return_raw)]
    pub fn mbox(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        rcpt: &str,
    ) -> EngineResult<()> {
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
    #[rhai_fn(global, return_raw)]
    pub fn mbox_all(this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Mbox,
        );

        Ok(())
    }

    /// set the delivery method to "Maildir" for a single recipient.
    #[rhai_fn(global, return_raw)]
    pub fn maildir(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        rcpt: &str,
    ) -> EngineResult<()> {
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
    #[rhai_fn(global, return_raw)]
    pub fn maildir_all(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
    ) -> EngineResult<()> {
        set_transport(
            &mut *this
                .write()
                .map_err::<Box<EvalAltResult>, _>(|_| "rule engine mutex poisoned".into())?,
            &vsmtp_common::transfer::Transfer::Maildir,
        );

        Ok(())
    }

    /// remove the delivery method for a specific recipient.
    #[rhai_fn(global, return_raw)]
    pub fn disable_delivery(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        rcpt: &str,
    ) -> EngineResult<()> {
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
    #[rhai_fn(global, return_raw)]
    pub fn disable_delivery_all(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
    ) -> EngineResult<()> {
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
        address::Address,
        rcpt::Rcpt,
        transfer::{ForwardTarget, Transfer},
    };

    #[test]
    fn test_set_transport_for() {
        let mut ctx = get_default_context();

        ctx.envelop.rcpt.push(Rcpt::new(
            Address::try_from("valid@rcpt.foo".to_string()).unwrap(),
        ));

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
    }
}
