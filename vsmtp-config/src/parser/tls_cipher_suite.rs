use vsmtp_common::re::anyhow;

/*
const ALL_CIPHER_SUITE: [rustls::CipherSuite; 9] = [
    // TLS1.3 suites
    rustls::CipherSuite::TLS13_AES_256_GCM_SHA384,
    rustls::CipherSuite::TLS13_AES_128_GCM_SHA256,
    rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
    // TLS1.2 suites
    rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
    rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
    rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
    rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
    rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
    rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
];
*/

struct CipherSuite(rustls::CipherSuite);

impl std::str::FromStr for CipherSuite {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "TLS_AES_256_GCM_SHA384" => Ok(Self(rustls::CipherSuite::TLS13_AES_256_GCM_SHA384)),
            "TLS_AES_128_GCM_SHA256" => Ok(Self(rustls::CipherSuite::TLS13_AES_128_GCM_SHA256)),
            "TLS_CHACHA20_POLY1305_SHA256" => {
                Ok(Self(rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256))
            }
            "ECDHE_ECDSA_WITH_AES_256_GCM_SHA384" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
            )),
            "ECDHE_ECDSA_WITH_AES_128_GCM_SHA256" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
            )),
            "ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
            )),
            "ECDHE_RSA_WITH_AES_256_GCM_SHA384" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
            )),
            "ECDHE_RSA_WITH_AES_128_GCM_SHA256" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
            )),
            "ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256" => Ok(Self(
                rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
            )),
            _ => Err(anyhow::anyhow!("not a valid cipher suite: '{}'", s)),
        }
    }
}

impl std::fmt::Display for CipherSuite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            rustls::CipherSuite::TLS13_AES_256_GCM_SHA384 => "TLS_AES_256_GCM_SHA384",
            rustls::CipherSuite::TLS13_AES_128_GCM_SHA256 => "TLS_AES_128_GCM_SHA256",
            rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256 => "TLS_CHACHA20_POLY1305_SHA256",
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384 => {
                "ECDHE_ECDSA_WITH_AES_256_GCM_SHA384"
            }
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256 => {
                "ECDHE_ECDSA_WITH_AES_128_GCM_SHA256"
            }
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256 => {
                "ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256"
            }
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384 => {
                "ECDHE_RSA_WITH_AES_256_GCM_SHA384"
            }
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256 => {
                "ECDHE_RSA_WITH_AES_128_GCM_SHA256"
            }
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256 => {
                "ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256"
            }
            _ => "unsupported",
        })
    }
}

impl serde::Serialize for CipherSuite {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{}", self))
    }
}

impl<'de> serde::Deserialize<'de> for CipherSuite {
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

pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<rustls::CipherSuite>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;

    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<CipherSuite>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("[...]")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut v = Vec::<Result<CipherSuite, A::Error>>::new();
            while let Some(i) = seq.next_element::<&str>()? {
                v.push(
                    <CipherSuite as std::str::FromStr>::from_str(i)
                        .map_err(serde::de::Error::custom),
                );
            }

            v.into_iter()
                .collect::<Result<Vec<CipherSuite>, A::Error>>()
        }
    }

    Ok(deserializer
        .deserialize_any(Visitor)?
        .into_iter()
        .map(|i| i.0)
        .collect::<Vec<_>>())
}

pub fn serialize<S>(this: &[rustls::CipherSuite], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let mut seq = serializer.serialize_seq(Some(this.len()))?;
    for i in this {
        serde::ser::SerializeSeq::serialize_element(&mut seq, &CipherSuite(*i))?;
    }
    serde::ser::SerializeSeq::end(seq)
}

#[cfg(test)]
mod tests {
    use crate::parser::tls_cipher_suite::CipherSuite;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct S {
        #[serde(
            serialize_with = "crate::parser::tls_cipher_suite::serialize",
            deserialize_with = "crate::parser::tls_cipher_suite::deserialize"
        )]
        v: Vec<rustls::CipherSuite>,
    }

    #[test]
    fn error() {
        assert!(toml::from_str::<S>(r#"v = ["SRP_SHA_WITH_AES_128_CBC_SHA"]"#).is_err());
        assert!(toml::from_str::<S>(r#"v = "foobar""#).is_err());
        assert!(toml::from_str::<S>(r#"v = 100"#).is_err());
    }

    #[test]
    fn tls1_3() {
        assert_eq!(
            toml::from_str::<S>(
                r#"v = [
    "TLS_AES_256_GCM_SHA384",
    "TLS_AES_128_GCM_SHA256",
    "TLS_CHACHA20_POLY1305_SHA256"
]"#
            )
            .unwrap()
            .v,
            vec![
                rustls::CipherSuite::TLS13_AES_256_GCM_SHA384,
                rustls::CipherSuite::TLS13_AES_128_GCM_SHA256,
                rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
            ]
        );
    }

    #[test]
    fn tls1_2() {
        assert_eq!(
            toml::from_str::<S>(
                r#"v = [
    "ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
    "ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
    "ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
    "ECDHE_RSA_WITH_AES_256_GCM_SHA384",
    "ECDHE_RSA_WITH_AES_128_GCM_SHA256",
    "ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
]"#
            )
            .unwrap()
            .v,
            vec![
                rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
                rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
                rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
                rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
                rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
                rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
            ]
        );
    }

    const ALL_CIPHER_SUITE: [rustls::CipherSuite; 9] = [
        rustls::CipherSuite::TLS13_AES_256_GCM_SHA384,
        rustls::CipherSuite::TLS13_AES_128_GCM_SHA256,
        rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
        rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
        rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
        rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
        rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
        rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
        rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
    ];

    #[test]
    fn serialize() {
        for i in ALL_CIPHER_SUITE {
            assert_eq!(
                serde_json::to_string(&S { v: vec![i] }).unwrap(),
                format!("{{\"v\":[\"{}\"]}}", CipherSuite(i))
            );
        }
    }
}
