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

use crate::dsl::service::SmtpConnection;
use crate::{api::EngineResult, dsl::service::Service};
use rhai::EvalAltResult;
use vsmtp_common::re::lettre;

pub fn parse_smtp_service(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
    service_name: &str,
) -> EngineResult<Service> {
    /// extract a value from a `rhai::Map`, optionally inserting a default value.
    fn get_or_default<T: Clone + Send + Sync + 'static>(
        map_name: &str,
        map: &rhai::Map,
        key: &str,
        default: Option<T>,
    ) -> EngineResult<T> {
        fn try_cast<T: Clone + Send + Sync + 'static>(
            name: &str,
            value: &rhai::Dynamic,
        ) -> EngineResult<T> {
            value
                .clone()
                .try_cast::<T>()
                .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                    format!(
                        "the {name} parameter for a smtp service must be a {}",
                        std::any::type_name::<T>()
                    )
                    .into()
                })
        }

        match (map.get(key), default) {
            (Some(value), _) => try_cast(key, value),
            (mut value, Some(default)) => {
                try_cast(key, value.get_or_insert(&rhai::Dynamic::from(default)))
            }
            _ => Err(format!("key {key} was not found in {map_name}").into()),
        }
    }

    let options: rhai::Map = context
        .eval_expression_tree(&input[3])?
        .try_cast()
        .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
            "smtp service options must be a map".into()
        })?;

    let receiver_addr = get_or_default::<String>(service_name, &options, "receiver", None)?
        .parse::<std::net::SocketAddr>()
        .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?;

    // TODO: add a 'unix'/'net' modifier.
    let delegator: rhai::Map = get_or_default(service_name, &options, "delegator", None)?;
    let delegator_addr = get_or_default::<String>("delegator", &delegator, "address", None)?
        .parse::<std::net::SocketAddr>()
        .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?;
    let delegator_timeout: std::time::Duration =
        get_or_default::<String>(service_name, &options, "timeout", Some("60s".to_string()))?
            .parse::<vsmtp_config::re::humantime::Duration>()
            .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?
            .into();

    Ok(Service::Smtp {
        delegator: {
            SmtpConnection(std::sync::Arc::new(std::sync::Mutex::new(
                lettre::SmtpTransport::builder_dangerous(delegator_addr.ip().to_string())
                    .port(delegator_addr.port())
                    .timeout(Some(delegator_timeout))
                    .build(),
            )))
        },
        receiver: receiver_addr,
    })
}
