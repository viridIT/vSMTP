use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};

#[rhai::plugin::export_module]
pub mod bcc {

    use crate::{modules::actions::MailContext, modules::EngineResult, obj::Object};
    use vsmtp_common::address::Address;

    /// add a recipient to the list recipient using a raw string.
    #[rhai_fn(global, name = "bcc", return_raw, pure)]
    pub fn from_str(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        bcc: &str,
    ) -> EngineResult<()> {
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
    pub fn from_addr(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        bcc: Address,
    ) -> EngineResult<()> {
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
    pub fn from_object(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        bcc: std::sync::Arc<Object>,
    ) -> EngineResult<()> {
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
