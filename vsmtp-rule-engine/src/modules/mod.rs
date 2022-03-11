use rhai::EvalAltResult;

pub(crate) mod actions;
pub(crate) mod mail_context;
pub(crate) mod types;

pub(crate) type EngineResult<T> = Result<T, Box<EvalAltResult>>;
