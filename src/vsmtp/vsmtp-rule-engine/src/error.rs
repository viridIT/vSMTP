/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub enum RuleEngineError {
    Object,
    Rule,
    Action,
    Stage,
}

impl RuleEngineError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Object => {
                r#"failed to parse an object.
    use the extended syntax:

    obj "type" "name" "value";

    or

    obj "type" "name" #{
        value: ...,
        ..., // any field are accepted using the extended syntax.
    };

    or use the inline syntax:

    obj "type" "name" "value";
"#
            }

            Self::Rule => {
                r#"failed to parse a rule.
    use the following syntax:

    rule "name" || {
        ... // your code to execute.
        sys::next() // must end with a status. (next, accept, faccept ...)
    },
"#
            }

            Self::Action => {
                r#"failed to parse an action.
    use the following syntax:

    action "name" || {
        ... // your code to execute.
    };
"#
            }

            Self::Stage => {
                r#"failed to parse a stage.
    declare stages this way:

    #{
        preq: [
            ...  // rules & actions
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
