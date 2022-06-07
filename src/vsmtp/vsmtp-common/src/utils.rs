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

use anyhow::Context;
use std::str::FromStr;

// TODO: handle the case when the port is not specified.
/// parse an ip6 string address containing a scope id.
/// NOTE: specifiyng a port is mendatory for this implementation,
///       since it is only used for toml interface config & forwarding.
///
/// # Errors
/// * port was not found.
/// * failed to parse the port.
/// * failed to parse the scope id.
pub fn ipv6_with_scope_id(input: &str) -> anyhow::Result<std::net::SocketAddr> {
    if ip6_has_scope_id(input) {
        let (addr, port) = parse_ip6_port(input)?;
        let (addr, scope_id) = parse_ip6_scope_id(addr)?;
        let mut socket = std::net::SocketAddrV6::from_str(&format!("[{addr}]:{port}"))?;

        socket.set_scope_id(crate::libc_abstraction::if_nametoindex(scope_id)?);

        Ok(std::net::SocketAddr::V6(socket))
    } else {
        Ok(std::net::SocketAddr::from_str(input)?)
    }
}

fn parse_ip6_port(input: &str) -> anyhow::Result<(&str, u16)> {
    let (addr, port) = input
        .rsplit_once(':')
        .context("could not parse ip6 address")?;

    Ok((
        addr.strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .ok_or_else(|| anyhow::anyhow!("ipv6 invalid format"))?,
        port.parse::<u16>().context("could not parse port of ip6")?,
    ))
}

fn parse_ip6_scope_id(input: &str) -> anyhow::Result<(&str, &str)> {
    input
        .rsplit_once('%')
        .context("could not parse ip6 address scope id")
}

fn ip6_has_scope_id(input: &str) -> bool {
    input.rfind('%').is_some()
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_ip6_with_scope_id() {
        // the function does not handle parsing without a port.
        assert!(ipv6_with_scope_id("::1").is_err(),);
        assert!(ipv6_with_scope_id("::1%eth0").is_err(),);

        assert_eq!(
            ipv6_with_scope_id("[::1]:25").unwrap(),
            std::net::SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 25)
        );
        // NOTE: I did not add an scope id test here because it changes between machines.
    }
}
