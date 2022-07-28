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
use crate::api::{
    EngineResult, {Message, SharedObject},
};
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};

pub use message_rhai::*;

#[rhai::plugin::export_module]
mod message_rhai {

    /// check if a given header exists in the top level headers.
    #[rhai_fn(global, name = "has_header", return_raw, pure)]
    pub fn has_header(message: &mut Message, header: &str) -> EngineResult<bool> {
        Ok(vsl_guard_ok!(message.read()).get_header(header).is_some())
    }

    /// return the value of a header if it exists. Otherwise, returns an empty string.
    #[rhai_fn(global, name = "get_header", return_raw, pure)]
    pub fn get_header(message: &mut Message, header: &str) -> EngineResult<String> {
        Ok(vsl_guard_ok!(message.read())
            .get_header(header)
            .unwrap_or_default())
    }

    /// Return a list of headers bearing the `name` given as argument.
    /// The `count` parameter specify the number of headers with the same name
    /// to return.
    #[rhai_fn(global, name = "get_headers", return_raw, pure)]
    pub fn get_headers_str(
        message: &mut Message,
        name: &str,
        count: rhai::INT,
    ) -> EngineResult<rhai::Dynamic> {
        super::get_headers(
            message,
            name,
            usize::try_from(count)
                .map_err::<Box<rhai::EvalAltResult>, _>(|e| e.to_string().into())?,
        )
    }

    /// Return a list of headers bearing the `name` given as argument.
    /// The `count` parameter specify the number of headers with the same name
    /// to return.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "get_headers", return_raw, pure)]
    pub fn get_headers_obj(
        message: &mut Message,
        name: SharedObject,
        count: rhai::INT,
    ) -> EngineResult<rhai::Dynamic> {
        super::get_headers(
            message,
            &name.to_string(),
            usize::try_from(count)
                .map_err::<Box<rhai::EvalAltResult>, _>(|e| e.to_string().into())?,
        )
    }

    /// add a header to the end of the raw or parsed email contained in ctx.
    #[rhai_fn(global, name = "append_header", return_raw, pure)]
    pub fn append_header_str_str(
        message: &mut Message,
        header: &str,
        value: &str,
    ) -> EngineResult<()> {
        super::append_header(message, &header, &value)
    }

    /// add a header to the end of the raw or parsed email contained in ctx. (using an object)
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "append_header", return_raw, pure)]
    pub fn append_header_str_obj(
        message: &mut Message,
        header: &str,
        value: SharedObject,
    ) -> EngineResult<()> {
        super::append_header(message, &header, &value.to_string())
    }

    /// prepend a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, name = "prepend_header", return_raw, pure)]
    pub fn prepend_header_str_str(
        message: &mut Message,
        header: &str,
        value: &str,
    ) -> EngineResult<()> {
        super::prepend_header(message, header, value)
    }

    /// prepend a header to the raw or parsed email contained in ctx. (using an object)
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, name = "prepend_header", return_raw, pure)]
    pub fn prepend_header_str_obj(
        message: &mut Message,
        header: &str,
        value: SharedObject,
    ) -> EngineResult<()> {
        super::prepend_header(message, header, &value.to_string())
    }

    /// set a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, name = "set_header", return_raw, pure)]
    pub fn set_header_str_str(
        message: &mut Message,
        header: &str,
        value: &str,
    ) -> EngineResult<()> {
        super::set_header(message, header, value)
    }

    /// set a header to the raw or parsed email contained in ctx.
    #[rhai_fn(global, name = "set_header", return_raw, pure)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn set_header_str_obj(
        message: &mut Message,
        header: &str,
        value: SharedObject,
    ) -> EngineResult<()> {
        super::set_header(message, header, &value.to_string())
    }

    /// Get the message body as a string
    #[rhai_fn(global, get = "mail", return_raw, pure)]
    pub fn mail(this: &mut Message) -> EngineResult<String> {
        Ok(vsl_guard_ok!(this.read()).inner().to_string())
    }
}

/// Return a list of headers bearing the `name` given as argument.
/// The `count` parameter specify the number of headers with the same name
/// to return.
fn get_headers(this: &mut Message, name: &str, count: usize) -> EngineResult<rhai::Dynamic> {
    let guard = vsl_guard_ok!(this.read());
    let name_lowercase = name.to_lowercase();

    Ok(guard
        .inner()
        .headers()
        .iter()
        .filter(|(key, _)| key.to_lowercase() == name_lowercase)
        .take(count)
        .map(|(key, value)| format!("{key}:{value}"))
        .collect::<Vec<_>>()
        .into())
}

/// internal generic function to append a header to the message.
fn append_header<T, U>(message: &mut Message, header: &T, value: &U) -> EngineResult<()>
where
    T: AsRef<str> + ?Sized,
    U: AsRef<str> + ?Sized,
{
    vsl_guard_ok!(message.write()).append_header(header.as_ref(), value.as_ref());
    Ok(())
}

/// internal generic function to prepend a header to the message.
fn prepend_header<T, U>(message: &mut Message, header: &T, value: &U) -> EngineResult<()>
where
    T: AsRef<str> + ?Sized,
    U: AsRef<str> + ?Sized,
{
    vsl_guard_ok!(message.write()).prepend_header(header.as_ref(), value.as_ref());
    Ok(())
}

/// internal generic function to set the value of a header.
fn set_header<T, U>(message: &mut Message, header: &T, value: &U) -> EngineResult<()>
where
    T: AsRef<str> + ?Sized,
    U: AsRef<str> + ?Sized,
{
    vsl_guard_ok!(message.write()).set_header(header.as_ref(), value.as_ref());
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::dsl::object::Object;
    use vsmtp_common::MessageBody;

    #[test]
    fn test_has_header_success() {
        let mut message = std::sync::Arc::new(std::sync::RwLock::new(MessageBody::default()));

        append_header_str_str(&mut message, "X-HEADER-1", "VALUE-1").unwrap();
        append_header_str_obj(
            &mut message,
            "X-HEADER-2",
            std::sync::Arc::new(Object::Str("VALUE-2".to_string())),
        )
        .unwrap();

        assert!(has_header(&mut message, "X-HEADER-1").unwrap());
        assert!(has_header(&mut message, "X-HEADER-2").unwrap());
        assert!(!has_header(&mut message, "X-HEADER-3").unwrap());
    }

    #[test]
    fn test_get_header_success() {
        let mut message = std::sync::Arc::new(std::sync::RwLock::new(MessageBody::default()));

        append_header_str_str(&mut message, "X-HEADER-1", "VALUE-1").unwrap();
        append_header_str_obj(
            &mut message,
            "X-HEADER-2",
            std::sync::Arc::new(Object::Str("VALUE-2".to_string())),
        )
        .unwrap();

        assert_eq!(get_header(&mut message, "X-HEADER-1").unwrap(), "VALUE-1");
        assert_eq!(get_header(&mut message, "X-HEADER-2").unwrap(), "VALUE-2");
        assert_eq!(get_header(&mut message, "X-HEADER-3").unwrap(), "");
    }

    #[test]
    fn test_append_header_success() {
        let mut message = std::sync::Arc::new(std::sync::RwLock::new(MessageBody::default()));

        append_header_str_str(&mut message, "X-HEADER-1", "VALUE-1").unwrap();
        append_header_str_obj(
            &mut message,
            "X-HEADER-2",
            std::sync::Arc::new(Object::Str("VALUE-2".to_string())),
        )
        .unwrap();

        assert_eq!(
            message.read().unwrap().get_header("X-HEADER-1").unwrap(),
            "VALUE-1"
        );
        assert_eq!(
            message.read().unwrap().get_header("X-HEADER-2").unwrap(),
            "VALUE-2"
        );
    }

    #[test]
    fn test_prepend_header_success() {
        let mut message = std::sync::Arc::new(std::sync::RwLock::new(MessageBody::default()));

        prepend_header_str_str(&mut message, "X-HEADER-1", "VALUE-1").unwrap();
        prepend_header_str_obj(
            &mut message,
            "X-HEADER-2",
            std::sync::Arc::new(Object::Str("VALUE-2".to_string())),
        )
        .unwrap();

        assert_eq!(
            message.read().unwrap().get_header("X-HEADER-1").unwrap(),
            "VALUE-1"
        );
        assert_eq!(
            message.read().unwrap().get_header("X-HEADER-2").unwrap(),
            "VALUE-2"
        );
    }

    #[test]
    fn test_set_header_success() {
        let mut message = std::sync::Arc::new(std::sync::RwLock::new(MessageBody::default()));

        set_header_str_str(&mut message, "X-HEADER", "VALUE-1").unwrap();
        assert_eq!(
            message.read().unwrap().get_header("X-HEADER").unwrap(),
            "VALUE-1"
        );

        set_header_str_obj(
            &mut message,
            "X-HEADER",
            std::sync::Arc::new(Object::Str("VALUE-2".to_string())),
        )
        .unwrap();
        assert_eq!(
            message.read().unwrap().get_header("X-HEADER").unwrap(),
            "VALUE-2"
        );
    }
}
