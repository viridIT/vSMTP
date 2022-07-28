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
use crate::{api::EngineResult, error::CompilationError};

pub fn parse_delegation(
    symbols: &[rhai::ImmutableString],
    look_ahead: &str,
) -> Result<Option<rhai::ImmutableString>, rhai::ParseError> {
    match symbols.len() {
        // the delegate keyword and the name of the delegation ...
        1 | 3 => Ok(Some("$expr$".into())),
        // service to use and function body.
        2 => Ok(Some("$string$".into())),
        4 => Ok(None),
        _ => Err(rhai::ParseError(
            Box::new(rhai::ParseErrorType::BadInput(
                rhai::LexError::UnexpectedInput(format!(
                    "Improper delegate declaration: keyword '{}' unknown.",
                    look_ahead
                )),
            )),
            rhai::Position::NONE,
        )),
    }
}

pub fn create_delegation(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
) -> EngineResult<rhai::Dynamic> {
    let service = context.eval_expression_tree(&input[0])?;
    let name = input[1]
        .get_literal_value::<rhai::ImmutableString>()
        .unwrap();
    let expr = context.eval_expression_tree(&input[2])?;

    Ok(rhai::Dynamic::from(
        [
            ("name".into(), rhai::Dynamic::from(name.clone())),
            ("service".into(), rhai::Dynamic::from(service)),
            ("type".into(), "delegate".into()),
        ]
        .into_iter()
        .chain(if expr.is::<rhai::Map>() {
            let properties = expr.cast::<rhai::Map>();

            if properties
                .get("evaluate")
                .filter(|f| f.is::<rhai::FnPtr>())
                .is_none()
            {
                return Err(
                    format!("'evaluate' function is missing from '{}' delegation", name).into(),
                );
            }

            properties.into_iter()
        } else if expr.is::<rhai::FnPtr>() {
            rhai::Map::from_iter([("evaluate".into(), expr)]).into_iter()
        } else {
            return Err(format!(
                "a delegation must be a rhai::Map (#{{}}) or an anonymous function (|| {{}}){}",
                CompilationError::Action.as_str()
            )
            .into());
        })
        .collect::<rhai::Map>(),
    ))
}
