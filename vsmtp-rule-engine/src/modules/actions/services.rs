use rhai::plugin::{
    mem, Dynamic, EvalAltResult, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, Position, RhaiResult, TypeId,
};

#[rhai::plugin::export_module]
pub mod services {
    use crate::{
        modules::actions::MailContext, modules::EngineResult, server_api::ServerAPI,
        service::ServiceResult,
    };

    /// execute the service named @service_name from the vSMTP configuration definition
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, return_raw)]
    pub fn run_service(
        srv: &mut std::sync::Arc<ServerAPI>,
        ctx: std::sync::Arc<std::sync::RwLock<MailContext>>,
        service_name: &str,
    ) -> EngineResult<ServiceResult> {
        crate::service::run(
            srv.config
                .app
                .services
                .get(service_name)
                .ok_or_else::<Box<EvalAltResult>, _>(|| {
                    format!("No service in config named: '{service_name}'").into()
                })?,
            &*ctx
                .read()
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?,
        )
        .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())
    }
}
