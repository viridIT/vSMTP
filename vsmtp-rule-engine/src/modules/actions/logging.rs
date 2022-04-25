use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};

#[rhai::plugin::export_module]
pub mod logging {
    use vsmtp_common::re::log;
    use vsmtp_config::log_channel::APP;

    ///
    pub fn log(level: &str, message: &str) {
        match level {
            "trace" => log::trace!(target: APP, "{}", message),
            "debug" => log::debug!(target: APP, "{}", message),
            "info" => log::info!(target: APP, "{}", message),
            "warn" => log::warn!(target: APP, "{}", message),
            "error" => log::error!(target: APP, "{}", message),
            unknown => log::warn!(
                target: APP,
                "'{}' is not a valid log level. Original message: '{}'",
                unknown,
                message
            ),
        }
    }
}
