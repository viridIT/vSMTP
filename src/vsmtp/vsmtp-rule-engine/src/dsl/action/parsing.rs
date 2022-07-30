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

pub fn parse_action(
    symbols: &[rhai::ImmutableString],
    look_ahead: &str,
) -> Result<Option<rhai::ImmutableString>, rhai::ParseError> {
    match symbols.len() {
        // action keyword ...
        1 => Ok(Some("$string$".into())),
        // name of the action ...
        2 => Ok(Some("$expr$".into())),
        // block, we are done parsing.
        3 => Ok(None),
        _ => Err(rhai::ParseError(
            Box::new(rhai::ParseErrorType::BadInput(
                rhai::LexError::UnexpectedInput(format!(
                    "Improper action declaration: keyword '{}' unknown.",
                    look_ahead
                )),
            )),
            rhai::Position::NONE,
        )),
    }
}

pub fn create_action(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
) -> EngineResult<rhai::Dynamic> {
    let name = input[0]
        .get_literal_value::<rhai::ImmutableString>()
        .unwrap();
    let expr = context.eval_expression_tree(&input[1])?;

    Ok(rhai::Dynamic::from(
        [
            ("name".into(), rhai::Dynamic::from(name.clone())),
            ("type".into(), "action".into()),
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
                    format!("'evaluate' function is missing from '{}' action", name).into(),
                );
            }

            properties.into_iter()
        } else if expr.is::<rhai::FnPtr>() {
            rhai::Map::from_iter([("evaluate".into(), expr)]).into_iter()
        } else {
            return Err(format!(
                "an action must be a rhai::Map (#{{}}) or an anonymous function (|| {{}}){}",
                CompilationError::Action.as_str()
            )
            .into());
        })
        .collect::<rhai::Map>(),
    ))
}
