use vsmtp_common::libc_abstraction::if_nametoindex;

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<std::net::SocketAddr>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    fn ipv6_with_scope_id(input: &str) -> anyhow::Result<std::net::SocketAddr> {
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

        socket_addr.set_scope_id(if_nametoindex(scope_name)?);
        Ok(std::net::SocketAddr::V6(socket_addr))
    }

    <Vec<String> as serde::Deserialize>::deserialize(deserializer)?
        .into_iter()
        .map(|s| {
            <std::net::SocketAddr as std::str::FromStr>::from_str(&s)
                .or_else(|_| ipv6_with_scope_id(&s))
        })
        .collect::<anyhow::Result<Vec<std::net::SocketAddr>>>()
        .map_err(serde::de::Error::custom)
}
