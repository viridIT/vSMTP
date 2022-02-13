use rhai::EvalAltResult;

pub mod actions;
pub mod email;
pub mod types;

pub type EngineResult<T> = Result<T, Box<EvalAltResult>>;
