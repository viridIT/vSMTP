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

use std::str::FromStr;

use crate::dsl::service::SmtpConnection;
use crate::{dsl::service::Service, modules::EngineResult};
use rhai::EvalAltResult;
use vsmtp_common::envelop::build_lettre;
use vsmtp_common::rcpt::Rcpt;
use vsmtp_common::re::anyhow;
use vsmtp_common::re::lettre::{self, Transport};
use vsmtp_common::state::StateSMTP;
use vsmtp_common::Address;

pub fn parse_smtp_service(
    context: &mut rhai::EvalContext,
    input: &[rhai::Expression],
    service_name: &str,
) -> EngineResult<Service> {
    /// extract a value from a rhai::Map, optionally inserting a default value.
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

    let run_on: String = get_or_default(service_name, &options, "run_on", None)?;

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
            std::sync::Arc::new(std::sync::Mutex::new(SmtpConnection(
                // std::net::TcpStream::connect(delegator_addr).unwrap(),
                lettre::SmtpTransport::builder_dangerous(delegator_addr.ip().to_string())
                    .port(delegator_addr.port())
                    .timeout(Some(delegator_timeout))
                    .build(),
            )))
        },
        receiver: receiver_addr,
        run_on: StateSMTP::from_str(&run_on)
            .map_err::<Box<EvalAltResult>, _>(|err| err.to_string().into())?,
    })
}

/// delegate security handling via the smtp protocol.
pub fn delegate(
    transport: &mut SmtpConnection,
    from: &Address,
    to: &[Rcpt],
    email: &[u8],
) -> anyhow::Result<()> {
    // fn read_buf(transport: &mut SmtpConnection) -> String {
    //     // reading code 354.
    //     let buf: &mut [u8] = &mut [0; 100];
    //     transport.0.read(buf).unwrap();

    //     std::str::from_utf8_mut(buf).unwrap().to_string()
    // }

    // dbg!("delegate");

    // dbg!(read_buf(transport));
    // transport.0.write_all(b"helo example.com\r\n");

    // transport
    //     .0
    //     .write_all(format!("MAIL FROM: <{}>\r\n", from.full()).as_bytes())
    //     .unwrap();
    // // dbg!(read_buf(transport));

    // for rcpt in to {
    //     transport
    //         .0
    //         .write_all(format!("RCPT TO: <{}>\r\n", rcpt.address.full()).as_bytes())
    //         .unwrap();
    //     // dbg!(read_buf(transport));
    // }
    // transport.0.write_all(b"DATA\r\n").unwrap();
    // // NOTE: could be useless because of the following read.
    // transport.0.flush().unwrap();
    // dbg!(read_buf(transport));

    // // FIXME: use a &str as parameter to prevent convertion to String here.
    // for line in email.lines() {
    //     let line = line.unwrap();
    //     dbg!(&line);
    //     transport
    //         .0
    //         .write_all(format!("{line}\r\n").as_bytes())
    //         .unwrap();
    // }
    // transport.0.write_all(b"\r\n.\r\n").unwrap();
    // transport.0.flush().unwrap();
    // dbg!(read_buf(transport));
    // dbg!("delegate sent!");
    // Ok(())

    let envelope = build_lettre(from, to)?;

    dbg!("send raw ...");
    dbg!(&transport);
    dbg!(transport.0.test_connection().unwrap());
    transport.0.send_raw(&envelope, email).unwrap();
    // .context("failed to delegate email security")?;
    dbg!("done");

    Ok(())
}
