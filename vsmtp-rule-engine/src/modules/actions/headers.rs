use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};

#[rhai::plugin::export_module]
pub mod headers {
    use crate::{modules::actions::MailContext, modules::EngineResult};
    use vsmtp_common::{address::Address, mail_context::Body};

    /// check if a given header exists in the top level headers.
    #[rhai_fn(global, return_raw, pure)]
    pub fn has_header(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        header: &str,
    ) -> EngineResult<bool> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .body
            .get_header(header)
            .is_some())
    }

    /// return the value of a header if it exists. Otherwise, returns an empty string.
    #[rhai_fn(global, return_raw, pure)]
    pub fn get_header(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        header: &str,
    ) -> EngineResult<String> {
        Ok(this
            .read()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .body
            .get_header(header)
            .map(std::string::ToString::to_string)
            .unwrap_or_default())
    }

    /// add a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw, pure)]
    pub fn add_header(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        header: &str,
        value: &str,
    ) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .body
            .add_header(header, value);

        Ok(())
    }

    /// set a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, return_raw, pure)]
    pub fn set_header(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        header: &str,
        value: &str,
    ) -> EngineResult<()> {
        this.write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?
            .body
            .set_header(header, value);
        Ok(())
    }

    /// change the sender of the mail
    #[rhai_fn(global, return_raw, pure)]
    pub fn rewrite_mail_from(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        new_addr: &str,
    ) -> EngineResult<()> {
        let new_addr =
            Address::try_from(new_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
                format!(
                    "could not rewrite mail_from with '{}' because it is not valid address",
                    new_addr,
                )
                .into()
            })?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        email.envelop.mail_from = new_addr.clone();

        match &mut email.body {
                Body::Empty => Err("failed to rewrite mail_from: the email has not been received yet. Use this method in postq or later.".into()),
                Body::Raw(_) => Err("failed to rewrite mail_from: the email has not been parsed yet. Use this method in postq or later.".into()),
                Body::Parsed(body) => {
                    body.rewrite_mail_from(new_addr.full());
                    Ok(())
                },
            }
    }

    /// change a recipient of the mail
    #[rhai_fn(global, return_raw, pure)]
    pub fn rewrite_rcpt(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        old_addr: &str,
        new_addr: &str,
    ) -> EngineResult<()> {
        let old_addr =
            Address::try_from(old_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
                format!(
                    "could not rewrite address '{}' because it is not valid address",
                    old_addr,
                )
                .into()
            })?;

        let new_addr =
            Address::try_from(new_addr.to_string()).map_err::<Box<EvalAltResult>, _>(|_| {
                format!(
                    "could not rewrite address '{}' with '{}' because it is not valid address",
                    old_addr, new_addr,
                )
                .into()
            })?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        if let Body::Empty | Body::Raw(_) = &email.body {
            return Err("failed to rewrite rcpt: the email has not been received or parsed yet. Use this method in postq or later.".into());
        }

        if let Some(index) = email
            .envelop
            .rcpt
            .iter()
            .position(|rcpt| rcpt.address == old_addr)
        {
            email
                .envelop
                .rcpt
                .push(vsmtp_common::rcpt::Rcpt::new(new_addr.clone()));
            email.envelop.rcpt.swap_remove(index);

            match &mut email.body {
                Body::Parsed(body) => {
                    body.rewrite_rcpt(old_addr.full(), new_addr.full());
                    Ok(())
                }
                _ => unreachable!(),
            }
        } else {
            Err(format!(
                "could not rewrite address '{}' because it does not resides in rcpt.",
                old_addr
            )
            .into())
        }
    }

    /// add a recipient to the mail
    #[rhai_fn(global, return_raw, pure)]
    pub fn add_rcpt(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        new_addr: &str,
    ) -> EngineResult<()> {
        println!("add rcpt");
        let new_addr = Address::try_from(new_addr.to_string()).map_err(|_| {
            format!(
                "'{}' could not be converted to a valid rcpt address",
                new_addr
            )
        })?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        email
            .envelop
            .rcpt
            .push(vsmtp_common::rcpt::Rcpt::new(new_addr.clone()));

        match &mut email.body {
                Body::Empty => Err("failed to rewrite rcpt: the email has not been received yet. Use this method in postq or later.".into()),
                Body::Raw(_) => Err("failed to rewrite rcpt: the email has not been parsed yet. Use this method in postq or later.".into()),
                Body::Parsed(body) => {
                    body.add_rcpt(new_addr.full());
                    Ok(())
                },
            }
    }

    /// remove a recipient to the mail
    #[rhai_fn(global, return_raw, pure)]
    pub fn remove_rcpt(
        this: &mut std::sync::Arc<std::sync::RwLock<MailContext>>,
        addr: &str,
    ) -> EngineResult<()> {
        let addr = Address::try_from(addr.to_string())
            .map_err(|_| format!("{} could not be converted to a valid rcpt address", addr))?;

        let email = &mut this
            .write()
            .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

        if let Body::Empty | Body::Raw(_) = &email.body {
            return Err("failed to remove rcpt: the email has not been received or parsed yet. Use this method in postq or later.".into());
        }

        if let Some(index) = email
            .envelop
            .rcpt
            .iter()
            .position(|rcpt| rcpt.address == addr)
        {
            email.envelop.rcpt.remove(index);
            match &mut email.body {
                Body::Parsed(body) => body.remove_rcpt(addr.full()),
                _ => unreachable!(),
            };
            Ok(())
        } else {
            Err(format!(
                "could not remove address '{}' because it does not resides in rcpt.",
                addr
            )
            .into())
        }
    }
}
