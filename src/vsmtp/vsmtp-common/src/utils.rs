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

/// Parse an ip6 string address containing an interface: `[fe80::f03c:91ff:fedf:75ee%eth0]:8080`
/// see <https://github.com/rust-lang/rust/issues/65976>
///
/// # Errors
///
/// * if the address is not valid.
pub fn ipv6_with_scope_id(input: &str) -> anyhow::Result<std::net::SocketAddr> {
    let (addr_ip_and_scope_name, colon_and_port) = input.split_at(
        input
            .rfind(':')
            .ok_or_else(|| anyhow::anyhow!("ipv6 port not provided"))?,
    );

    let (addr_ip, scope_name) = addr_ip_and_scope_name
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| anyhow::anyhow!("ipv6 not valid format"))?
        .split_once('%')
        .ok_or_else(|| anyhow::anyhow!("ipv6 no scope_id"))?;

    let mut socket_addr = format!("[{addr_ip}]{colon_and_port}")
        .parse::<std::net::SocketAddrV6>()
        .map_err(|e| anyhow::anyhow!("ipv6 parser produce error: '{e}'"))?;

    socket_addr.set_scope_id(crate::libc_abstraction::if_nametoindex(scope_name)?);
    Ok(std::net::SocketAddr::V6(socket_addr))
}
