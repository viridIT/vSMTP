const ALL_PROTOCOL_VERSION: [rustls::ProtocolVersion; 2] = [
    rustls::ProtocolVersion::TLSv1_2,
    rustls::ProtocolVersion::TLSv1_3,
];

#[derive(PartialEq, Eq)]
struct ProtocolVersion(rustls::ProtocolVersion);

impl std::str::FromStr for ProtocolVersion {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "TLSv1.2" | "0x0303" => Ok(Self(rustls::ProtocolVersion::TLSv1_2)),
            "TLSv1.3" | "0x0304" => Ok(Self(rustls::ProtocolVersion::TLSv1_3)),
            _ => Err(anyhow::anyhow!("not a valid protocol version: '{}'", s)),
        }
    }
}

impl serde::Serialize for ProtocolVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self.0 {
            rustls::ProtocolVersion::TLSv1_2 => "TLSv1.2",
            rustls::ProtocolVersion::TLSv1_3 => "TLSv1.3",
            _ => todo!(),
        })
    }
}

impl<'de> serde::Deserialize<'de> for ProtocolVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <Self as std::str::FromStr>::from_str(&<String as serde::Deserialize>::deserialize(
            deserializer,
        )?)
        .map_err(serde::de::Error::custom)
    }
}

fn custom_deserialize_vec<'de, D>(deserializer: D) -> Result<Vec<ProtocolVersion>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct ProtocolVersionVisitor;

    impl<'de> serde::de::Visitor<'de> for ProtocolVersionVisitor {
        type Value = Vec<ProtocolVersion>;

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
                        .map_err(serde::de::Error::custom)?;

                    let (min_value_idx, _) = ALL_PROTOCOL_VERSION
                        .into_iter()
                        .enumerate()
                        .find(|(_, i)| *i == min_value.0)
                        .ok_or_else(|| {
                            serde::de::Error::custom(format!(
                                "not supported version: {:?}",
                                min_value.0
                            ))
                        })?;

                    Ok(ALL_PROTOCOL_VERSION[min_value_idx..]
                        .iter()
                        .copied()
                        .map(ProtocolVersion)
                        .collect())
                }
                None => Ok(vec![<ProtocolVersion as std::str::FromStr>::from_str(v)
                    .map_err(serde::de::Error::custom)?]),
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
                        .map_err(serde::de::Error::custom),
                );
            }

            v.into_iter()
                .collect::<Result<Vec<ProtocolVersion>, A::Error>>()
        }
    }

    deserializer.deserialize_any(ProtocolVersionVisitor)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<rustls::ProtocolVersion>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(custom_deserialize_vec(deserializer)?
        .iter()
        .map(|i| i.0)
        .collect())
}

pub fn serialize<S>(this: &[rustls::ProtocolVersion], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut seq = serializer.serialize_seq(Some(this.len()))?;
    for i in this {
        serde::ser::SerializeSeq::serialize_element(&mut seq, &ProtocolVersion(*i))?;
    }
    serde::ser::SerializeSeq::end(seq)
}

#[cfg(test)]
mod tests {

    #[derive(Debug, serde::Deserialize)]
    struct S {
        #[serde(
            serialize_with = "crate::parser::tls_protocol_version::serialize",
            deserialize_with = "crate::parser::tls_protocol_version::deserialize"
        )]
        v: Vec<rustls::ProtocolVersion>,
    }

    #[test]
    fn one_string() {
        let s = toml::from_str::<S>(r#"v = "TLSv1.2""#).unwrap();
        assert_eq!(s.v, vec![rustls::ProtocolVersion::TLSv1_2]);
        let s = toml::from_str::<S>(r#"v = "0x0303""#).unwrap();
        assert_eq!(s.v, vec![rustls::ProtocolVersion::TLSv1_2]);

        let s = toml::from_str::<S>(r#"v = "TLSv1.3""#).unwrap();
        assert_eq!(s.v, vec![rustls::ProtocolVersion::TLSv1_3]);
        let s = toml::from_str::<S>(r#"v = "0x0304""#).unwrap();
        assert_eq!(s.v, vec![rustls::ProtocolVersion::TLSv1_3]);
    }

    #[test]
    fn array() {
        let s = toml::from_str::<S>(r#"v = ["TLSv1.2", "TLSv1.3"]"#).unwrap();
        assert_eq!(
            s.v,
            vec![
                rustls::ProtocolVersion::TLSv1_2,
                rustls::ProtocolVersion::TLSv1_3,
            ]
        );
    }

    #[test]
    fn pattern() {
        let s = toml::from_str::<S>(r#"v = "^TLSv1.2""#).unwrap();
        assert_eq!(
            s.v,
            vec![
                rustls::ProtocolVersion::TLSv1_2,
                rustls::ProtocolVersion::TLSv1_3,
            ]
        );

        let s = toml::from_str::<S>(r#"v = ">=TLSv1.2""#).unwrap();
        assert_eq!(
            s.v,
            vec![
                rustls::ProtocolVersion::TLSv1_2,
                rustls::ProtocolVersion::TLSv1_3,
            ]
        );
    }
}
