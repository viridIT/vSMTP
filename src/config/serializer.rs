use super::server_config::{ProtocolVersion, ProtocolVersionRequirement};

const ALL_PROTOCOL_VERSION: [ProtocolVersion; 6] = [
    ProtocolVersion(rustls::ProtocolVersion::SSLv2),
    ProtocolVersion(rustls::ProtocolVersion::SSLv3),
    ProtocolVersion(rustls::ProtocolVersion::TLSv1_0),
    ProtocolVersion(rustls::ProtocolVersion::TLSv1_1),
    ProtocolVersion(rustls::ProtocolVersion::TLSv1_2),
    ProtocolVersion(rustls::ProtocolVersion::TLSv1_3),
];

impl std::str::FromStr for ProtocolVersion {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SSLv2" | "0x0200" => Ok(ProtocolVersion(rustls::ProtocolVersion::SSLv2)),
            "SSLv3" | "0x0300" => Ok(ProtocolVersion(rustls::ProtocolVersion::SSLv3)),
            "TLSv1.0" | "0x0301" => Ok(ProtocolVersion(rustls::ProtocolVersion::TLSv1_0)),
            "TLSv1.1" | "0x0302" => Ok(ProtocolVersion(rustls::ProtocolVersion::TLSv1_1)),
            "TLSv1.2" | "0x0303" => Ok(ProtocolVersion(rustls::ProtocolVersion::TLSv1_2)),
            "TLSv1.3" | "0x0304" => Ok(ProtocolVersion(rustls::ProtocolVersion::TLSv1_3)),
            _ => Err(()),
        }
    }
}

impl<'de> serde::Deserialize<'de> for ProtocolVersionRequirement {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ProtocolVersionVisitor;

        impl<'de> serde::de::Visitor<'de> for ProtocolVersionVisitor {
            type Value = ProtocolVersionRequirement;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("[...]")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match v.strip_prefix(">=").or_else(|| v.strip_prefix('^')) {
                    Some(v) => {
                        let min_value = <ProtocolVersion as std::str::FromStr>::from_str(v)
                            .map_err(|_| serde::de::Error::custom("invalid"))?;
                        let mut min_value_idx = 0;
                        for (idx, i) in ALL_PROTOCOL_VERSION.iter().enumerate() {
                            if *i == min_value {
                                min_value_idx = idx;
                            }
                        }

                        Ok(ProtocolVersionRequirement(
                            ALL_PROTOCOL_VERSION[min_value_idx..].to_vec(),
                        ))
                    }
                    None => Ok(ProtocolVersionRequirement(vec![
                        <ProtocolVersion as std::str::FromStr>::from_str(v)
                            .map_err(|_| serde::de::Error::custom("invalid"))?,
                    ])),
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut v = Vec::<Result<ProtocolVersion, A::Error>>::new();
                while let Some(i) = seq.next_element::<&str>()? {
                    v.push(
                        <ProtocolVersion as std::str::FromStr>::from_str(i)
                            .map_err(|_| serde::de::Error::custom("invalid")),
                    );
                }

                Ok(ProtocolVersionRequirement(v.into_iter().collect::<Result<
                    Vec<ProtocolVersion>,
                    A::Error,
                >>(
                )?))
            }
        }

        deserializer.deserialize_any(ProtocolVersionVisitor)
    }
}
