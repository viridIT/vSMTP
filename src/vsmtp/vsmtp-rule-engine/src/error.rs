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
use vsmtp_common::re::anyhow;

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub enum CompilationError {
    Object,
    Rule,
    Action,
    Stage,
}

impl CompilationError {
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

impl std::fmt::Display for CompilationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::error::Error for CompilationError {}

impl From<CompilationError> for Box<rhai::EvalAltResult> {
    fn from(err: CompilationError) -> Self {
        err.as_str().into()
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("a lock guard is poisoned: `{source}`")]
    PoisonedGuard { source: anyhow::Error },
    #[error("the field: `{field}` is not defined yet")]
    MissingField { field: String },
    #[error("failed to parse the message body, `{source}`")]
    ParseMessageBody { source: anyhow::Error },
    #[error("invalid type conversion expected: `{r#type}`, but got: `{source}`")]
    TypeError {
        r#type: &'static str,
        source: anyhow::Error,
    },
}

macro_rules! vsl_guard_ok {
    ($guard:expr) => {
        $guard.map_err::<Box<EvalAltResult>, _>(|e| {
            $crate::error::RuntimeError::PoisonedGuard {
                source: vsmtp_common::re::anyhow::anyhow!("{e}"),
            }
            .into()
        })?
    };
}

macro_rules! vsl_missing_ok {
    ($option:expr, $field:expr) => {
        $option
            .as_ref()
            .ok_or_else(|| $crate::error::RuntimeError::MissingField {
                field: $field.to_string(),
            })?
    };
    (mut $option:expr, $field:expr) => {
        $option
            .as_mut()
            .ok_or_else(|| $crate::error::RuntimeError::MissingField {
                field: $field.to_string(),
            })?
    };
}

macro_rules! vsl_parse_ok {
    ($writer:expr) => {{
        let message = vsl_missing_ok!($writer, "message");
        if !matches!(&message, MessageBody::Parsed(..)) {
            *$writer = Some(
                message
                    .to_parsed::<vsmtp_mail_parser::MailMimeParser>()
                    .map_err(|source| $crate::error::RuntimeError::ParseMessageBody { source })?,
            );
        }
        vsl_missing_ok!(mut $writer, "message")
    }};
}

macro_rules! vsl_conversion_ok {
    ($type_:expr, $result:expr) => {
        $result.map_err(|source| $crate::error::RuntimeError::TypeError {
            r#type: $type_,
            source,
        })?
    };
}

impl From<RuntimeError> for Box<rhai::EvalAltResult> {
    fn from(err: RuntimeError) -> Self {
        err.to_string().into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_error_formatting() {
        println!("{}", CompilationError::Object);
        println!("{}", CompilationError::Rule);
        println!("{}", CompilationError::Action);
        println!("{}", CompilationError::Stage);
    }

    #[test]
    fn test_error_from_rhai_error() {
        let rhai_err: Box<rhai::EvalAltResult> = CompilationError::Rule.into();
        println!("{}", rhai_err);
    }
}
