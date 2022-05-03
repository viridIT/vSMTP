use rhai::exported_module;
use rhai::EvalAltResult;

pub(crate) mod actions;
pub(crate) mod mail_context;
pub(crate) mod types;

pub(crate) type EngineResult<T> = Result<T, Box<EvalAltResult>>;

rhai::def_package! {
    /// vsl's standard api.
    pub StandardVSLPackage(module) {
        rhai::packages::StandardPackage::init(module);

        module.combine(exported_module!(super::modules::actions::bcc::bcc))
            .combine(exported_module!(super::modules::actions::headers::headers))
            .combine(exported_module!(super::modules::actions::logging::logging))
            .combine(exported_module!(super::modules::actions::rule_state::rule_state))
            .combine(exported_module!(super::modules::actions::services::services))
            .combine(exported_module!(super::modules::actions::transports::transports))
            .combine(exported_module!(super::modules::actions::utils::utils))
            .combine(exported_module!(super::modules::actions::write::write))
            .combine(exported_module!(super::modules::types::types))
            .combine(exported_module!(super::modules::mail_context::mail_context));
    }
}
