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
            // then a separator for the database type.
            "db" => Ok(Some(":".into())),
            // then options for the service
            "shell" => Ok(Some("$expr$".into())),
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
        4 => match look_ahead {
            // the service is a database, the next token will be the type.
            ":" => Ok(Some("$ident$".into())),
            // options as a rhai::Map, we are done parsing.
            _ => Ok(None),
        },
        5 => match look_ahead {
            // database types, then the service options.
            "csv" => Ok(Some("$expr$".into())),
            db_type => Err(rhai::ParseError(
                Box::new(rhai::ParseErrorType::BadInput(
                    rhai::LexError::ImproperSymbol(
                        db_type.into(),
                        format!("Unknown database type '{}'.", db_type),
                    ),
                )),
                rhai::Position::NONE,
            )),
        },
        6 => Ok(None),
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
        "db" => open_database(context, input, &service_name),
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
fn open_database(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
    service_name: &str,
) -> EngineResult<rhai::Map> {
    let database_type = input[3].get_string_value().unwrap();
    let options = context.eval_expression_tree(&input[4])?;

    if options.is::<rhai::Map>() {
        let mut options: rhai::Map = options
            .try_cast()
            .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                "database options must be declared with a map #{}".into()
            })?;

        options.insert("name".into(), rhai::Dynamic::from(service_name.to_string()));

        let options = match database_type {
            "csv" => super::databases::csv::parse_csv_database(service_name, options)?,
            _ => todo!(),
        };

        Ok(options)
    } else {
        Err(rhai::EvalAltResult::ErrorMismatchDataType(
            "Map".to_string(),
            options.type_name().to_string(),
            rhai::Position::NONE,
        )
        .into())
    }
}
