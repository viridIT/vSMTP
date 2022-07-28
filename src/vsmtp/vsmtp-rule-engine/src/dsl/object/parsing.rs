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
use super::Object;
use crate::api::EngineResult;
use crate::error::CompilationError;

/// check of a "object" expression is valid.
/// the syntax is:
///   `object $name$ $type[:file_type]$ = #{ value: "...", ... };`
///   `object $name$ $type[:file_type]$ = "...";`
pub fn parse_object(
    symbols: &[rhai::ImmutableString],
    look_ahead: &str,
) -> Result<Option<rhai::ImmutableString>, rhai::ParseError> {
    match symbols.len() {
        // object keyword, then the name of the object.
        1 | 2 => Ok(Some("$ident$".into())),
        // type of the object.
        3 => match symbols[2].as_str() {
            // regular type, next is the '=' token or ':' token in case of the file type.
            "ip4" | "ip6" | "rg4" | "rg6" | "fqdn" | "address" | "identifier" | "string"
            | "regex" | "group" | "file" | "code" => Ok(Some("$symbol$".into())),
            entry => Err(rhai::ParseError(
                Box::new(rhai::ParseErrorType::BadInput(
                    rhai::LexError::ImproperSymbol(
                        entry.into(),
                        format!("Improper object type '{}'.", entry),
                    ),
                )),
                rhai::Position::NONE,
            )),
        },
        4 => match symbols[3].as_str() {
            // ':' token for a file content type, next is the content type of the file.
            ":" => Ok(Some("$ident$".into())),
            // '=' token for another object type, next is the content of the object.
            "=" => Ok(Some("$expr$".into())),
            entry => Err(rhai::ParseError(
                Box::new(rhai::ParseErrorType::BadInput(
                    rhai::LexError::ImproperSymbol(
                        entry.into(),
                        "Improper symbol when parsing object".to_string(),
                    ),
                )),
                rhai::Position::NONE,
            )),
        },
        5 => match symbols[4].as_str() {
            // NOTE: could it be possible to add a "file" | "code" | "group" content type ?
            // content types handled by the file type. next is the '=' token.
            "ip4" | "ip6" | "rg4" | "rg6" | "fqdn" | "address" | "identifier" | "string"
            | "regex" => Ok(Some("=".into())),
            // an expression, in the case of a regular object, whe are done parsing.
            _ => Ok(None),
        },
        6 => match symbols[5].as_str() {
            // the '=' token, next is the path to the file or a map with it's value.
            "=" => Ok(Some("$expr$".into())),
            // map or string value for a regular type, we are done parsing.
            _ => Ok(None),
        },
        7 => Ok(None),
        _ => Err(rhai::ParseError(
            Box::new(rhai::ParseErrorType::BadInput(
                rhai::LexError::UnexpectedInput(format!(
                    "Improper object declaration: keyword '{}' unknown.",
                    look_ahead
                )),
            )),
            rhai::Position::NONE,
        )),
    }
}

/// parses the given syntax tree and construct an object from it. always called after `parse_object`.
pub fn create_object(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
) -> EngineResult<rhai::Dynamic> {
    let object_name = input[0].get_string_value().unwrap().to_string();
    let object_type = input[1].get_string_value().unwrap().to_string();

    let object = match object_type.as_str() {
        "file" => create_file(context, input, &object_name),
        "code" => create_code(context, input, &object_name),
        _ => create_other(context, input, &object_type, &object_name),
    }?;

    // FIXME: I know it's dirty, could be refactored.
    // NOTE: string objects are pushed as regular strings to reduce
    //       complexity in vsl's api parameters.
    match Object::from_map(&object)
        .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?
    {
        Object::Str(string) => {
            // Pushing object in scope, preventing a "let _" statement,
            // and returning a reference to the object in case of a parent group.
            // Also, exporting the variable by default using `set_alias`.
            context
                .scope_mut()
                .push_constant(&object_name, string.clone())
                .set_alias(object_name, "");

            Ok(rhai::Dynamic::from(string))
        }
        other => {
            let object_ptr = std::sync::Arc::new(other);

            // Pushing object in scope, preventing a "let _" statement,
            // and returning a reference to the object in case of a parent group.
            // Also, exporting the variable by default using `set_alias`.
            context
                .scope_mut()
                .push_constant(&object_name, object_ptr.clone())
                .set_alias(object_name, "");

            Ok(rhai::Dynamic::from(object_ptr))
        }
    }
}

/// create a file object as a Map.
fn create_file(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
    object_name: &str,
) -> EngineResult<rhai::Map> {
    let content_type = input[3].get_string_value().unwrap();
    let object = context.eval_expression_tree(&input[4])?;

    if object.is::<rhai::Map>() {
        let mut object: rhai::Map = object.try_cast().ok_or(CompilationError::Object)?;
        object.insert("type".into(), rhai::Dynamic::from("file"));
        object.insert("name".into(), rhai::Dynamic::from(object_name.to_string()));
        object.insert(
            "content_type".into(),
            rhai::Dynamic::from(content_type.to_string()),
        );
        Ok(object)
    } else if object.is::<String>() {
        let mut map = rhai::Map::new();
        map.insert("type".into(), rhai::Dynamic::from("file"));
        map.insert("name".into(), rhai::Dynamic::from(object_name.to_string()));
        map.insert(
            "content_type".into(),
            rhai::Dynamic::from(content_type.to_string()),
        );
        map.insert("value".into(), object);
        Ok(map)
    } else {
        return Err(rhai::EvalAltResult::ErrorMismatchDataType(
            "Map | String".to_string(),
            object.type_name().to_string(),
            rhai::Position::NONE,
        )
        .into());
    }
}

/// create a code object.
fn create_code(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
    object_name: &str,
) -> EngineResult<rhai::Map> {
    let object = context.eval_expression_tree(&input[3])?;

    if object.is::<String>() {
        let mut map = rhai::Map::new();
        map.insert("type".into(), rhai::Dynamic::from("code"));
        map.insert("name".into(), rhai::Dynamic::from(object_name.to_string()));
        map.insert("value".into(), object);
        Ok(map)
    } else if object.is::<rhai::Map>() {
        let mut object: rhai::Map = object.try_cast().ok_or(CompilationError::Object)?;

        for key in ["code", "enhanced", "text"] {
            if !object.contains_key(key) {
                return Err(
                    format!("code object {object_name} is missing the '{key}' key.").into(),
                );
            }
        }

        object.insert("type".into(), rhai::Dynamic::from("code".to_string()));
        object.insert("name".into(), rhai::Dynamic::from(object_name.to_string()));
        Ok(object)
    } else {
        return Err(rhai::EvalAltResult::ErrorMismatchDataType(
            "Map".to_string(),
            object.type_name().to_string(),
            rhai::Position::NONE,
        )
        .into());
    }
}

/// create a type other than file as a Map.
fn create_other(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
    object_type: &str,
    object_name: &str,
) -> EngineResult<rhai::Map> {
    let object = context.eval_expression_tree(&input[3])?;

    if object.is::<rhai::Map>() {
        let mut object: rhai::Map = object.try_cast().ok_or(CompilationError::Object)?;
        object.insert("type".into(), rhai::Dynamic::from(object_type.to_string()));
        object.insert("name".into(), rhai::Dynamic::from(object_name.to_string()));
        Ok(object)
    } else if object.is::<String>() || object.is::<rhai::Array>() {
        let mut map = rhai::Map::new();
        map.insert("type".into(), rhai::Dynamic::from(object_type.to_string()));
        map.insert("name".into(), rhai::Dynamic::from(object_name.to_string()));
        map.insert("value".into(), object);
        Ok(map)
    } else {
        return Err(rhai::EvalAltResult::ErrorMismatchDataType(
            "Map | String".to_string(),
            object.type_name().to_string(),
            rhai::Position::NONE,
        )
        .into());
    }
}
