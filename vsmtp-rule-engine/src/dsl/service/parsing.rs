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
use crate::modules::EngineResult;
use vsmtp_config::Service;

/// parse a service using rhai's parser.
pub fn parse_service(
    symbols: &[rhai::ImmutableString],
    look_ahead: &str,
) -> Result<Option<rhai::ImmutableString>, rhai::ParseError> {
    match symbols.len() {
        // 'service' keyword, then the name of the service.
        1 | 2 => Ok(Some("$ident$".into())),
        // type of the service.
        3 => match look_ahead {
            // then options for the service
            "file" => Ok(Some("$expr$".into())),
            entry => Err(rhai::ParseError(
                Box::new(rhai::ParseErrorType::BadInput(
                    rhai::LexError::ImproperSymbol(
                        entry.into(),
                        format!("Improper service type '{}'.", entry),
                    ),
                )),
                rhai::Position::NONE,
            )),
        },
        // options as a rhai::Map, we are done parsing.
        4 => Ok(None),
        _ => Err(rhai::ParseError(
            Box::new(rhai::ParseErrorType::BadInput(
                rhai::LexError::UnexpectedInput(format!(
                    "Improper service declaration: keyword '{}' unknown.",
                    look_ahead
                )),
            )),
            rhai::Position::NONE,
        )),
    }
}

/// parses the given syntax tree and construct a service from it.
pub fn create_service(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
) -> EngineResult<rhai::Dynamic> {
    let service_name = input[0].get_string_value().unwrap().to_string();
    let service_type = input[1].get_string_value().unwrap().to_string();

    let service = match service_type.as_str() {
        "file" => open_file(context, input, &service_name),
        _ => todo!(),
    }?;

    // let object_ptr = std::sync::Arc::new(
    //     Object::from(&object)
    //         .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?,
    // );

    // // Pushing object in scope, preventing a "let _" statement,
    // // and returning a reference to the object in case of a parent group.
    // // Also, exporting the variable by default using `set_alias`.
    // context
    //     .scope_mut()
    //     .push_constant(&object_name, object_ptr.clone())
    //     .set_alias(object_name, "");

    Ok(rhai::Dynamic::from(service))
}

/// open a file database using the csv crate.
fn open_file(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
    service_name: &str,
) -> EngineResult<rhai::Map> {
    let service = context.eval_expression_tree(&input[3])?;

    if service.is::<rhai::Map>() {
        let mut service: rhai::Map = service
            .try_cast()
            .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                "file database options must be declared with a map #{}".into()
            })?;

        for key in ["connector"] {
            if !service.contains_key(key) {
                return Err(format!("service {service_name} is missing the '{key}' key.").into());
            }
        }

        service.insert("type".into(), rhai::Dynamic::from("service".to_string()));
        service.insert("name".into(), rhai::Dynamic::from(service_name.to_string()));

        Ok(service)
    } else {
        Err(rhai::EvalAltResult::ErrorMismatchDataType(
            "Map".to_string(),
            service.type_name().to_string(),
            rhai::Position::NONE,
        )
        .into())
    }
}
