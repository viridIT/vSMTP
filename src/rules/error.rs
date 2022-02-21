#[derive(Debug)]
pub enum RuleEngineError {
    Object,
    Rule,
    Action,
    Stage,
}

impl RuleEngineError {
    pub fn as_str(&self) -> &'static str {
        match self {
            RuleEngineError::Object => {
                r#"failed to parse an object.
    use the extended syntax:

    obj "type" "name" #{
        value: ...,
    };

    or use the inline syntax:

    obj "type" "name" "value";
"#
            }

            RuleEngineError::Rule => {
                r#"failed to parse a rule.
    use the following syntax:

    rule "name" #{
        condition: || { ... }, # must be a boolean result.
        on_success: || { ... }, # must return a status. (CONTINUE, ACCEPT ...)
        on_failure: || { ... }, # same as above.
    };
"#
            }

            RuleEngineError::Action => {
                r#"failed to parse an action.
    use the following syntax:

    action "name" || {
        ... # your code to execute. (LOG, QUARANTINE ...)
    };
"#
            }

            RuleEngineError::Stage => {
                r#"failed to parse a stage.

    declare stages this way:

    #{
        preq: [
            ... rules
            ... action
        ],

        delivery: [
            ...
        ]
    }
"#
            }
        }
    }
}

impl std::fmt::Display for RuleEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::error::Error for RuleEngineError {}

impl From<RuleEngineError> for Box<rhai::EvalAltResult> {
    fn from(err: RuleEngineError) -> Self {
        err.as_str().into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_error_formatting() {
        println!("{}", RuleEngineError::Object);
        println!("{}", RuleEngineError::Rule);
        println!("{}", RuleEngineError::Action);
        println!("{}", RuleEngineError::Stage);
    }

    #[test]
    fn test_error_from_rhai_error() {
        let rhai_err: Box<rhai::EvalAltResult> = RuleEngineError::Rule.into();
        println!("{}", rhai_err);
    }
}
