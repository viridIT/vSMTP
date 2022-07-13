#[allow(clippy::module_name_repetitions)]
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("argument `{arg_name}` type in function `{func_name}` isn't correct: expected `{expected_types:?}`, but got `{given_type}`")]
    ArgumentError {
        func_name: &'static str,
        arg_name: &'static str,
        expected_types: Vec<&'static str>,
        given_type: &'static str,
    },
}

impl From<EngineError> for Box<rhai::EvalAltResult> {
    fn from(err: EngineError) -> Self {
        err.to_string().into()
    }
}
