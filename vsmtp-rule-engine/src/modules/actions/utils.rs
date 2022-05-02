use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};

#[rhai::plugin::export_module]
pub mod utils {

    use crate::modules::EngineResult;

    // TODO: not yet functional, the relayer cannot connect to servers.
    /// send a mail from a template.
    #[rhai_fn(return_raw)]
    pub fn send_mail(from: &str, to: rhai::Array, path: &str, relay: &str) -> EngineResult<()> {
        // TODO: email could be cached using an object. (obj mail "my_mail" "/path/to/mail")
        let email =
            std::fs::read_to_string(path).map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                format!("failed to load email at {path}: {err:?}").into()
            })?;

        let envelop = lettre::address::Envelope::new(
            Some(from.parse().map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                format!("sys::send_mail from parsing failed: {err:?}").into()
            })?),
            to.into_iter()
                // NOTE: address that couldn't be converted will be silently dropped.
                .filter_map(|rcpt| {
                    rcpt.try_cast::<String>()
                        .and_then(|s| s.parse::<lettre::Address>().map(Some).unwrap_or(None))
                })
                .collect(),
        )
        .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
            format!("sys::send_mail envelop parsing failed {err:?}").into()
        })?;

        match lettre::Transport::send_raw(
            &lettre::SmtpTransport::relay(relay)
                .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                    format!("sys::send_mail failed to connect to relay: {err:?}").into()
                })?
                .build(),
            &envelop,
            email.as_bytes(),
        ) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("sys::send_mail failed to send: {err:?}").into()),
        }
    }

    // TODO: use UsersCache to optimize user lookup.
    /// use the user cache to check if a user exists on the system.
    #[must_use]
    pub fn user_exist(name: &str) -> bool {
        vsmtp_config::re::users::get_user_by_name(name).is_some()
    }

    /// get the hostname of the machine.
    #[rhai_fn(return_raw)]
    pub fn hostname() -> EngineResult<String> {
        hostname::get()
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                format!("failed to get system's hostname: {err}").into()
            })?
            .to_str()
            .map_or(
                Err("the system's hostname is not UTF-8 valide".into()),
                |host| Ok(host.to_string()),
            )
    }
}
