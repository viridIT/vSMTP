use rhai::EvalAltResult;

pub mod actions;
pub mod email;
pub mod types;

type EngineResult<T> = Result<T, Box<EvalAltResult>>;
