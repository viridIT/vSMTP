pub fn deserialize<'de, D>(deserializer: D) -> Result<rustls::PrivateKey, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct PrivateKeyVisitor;

    impl<'de> serde::de::Visitor<'de> for PrivateKeyVisitor {
        type Value = rustls::PrivateKey;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("[...]")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let path = std::path::Path::new(v);
            if path.exists() {
                let mut reader = std::io::BufReader::new(
                    std::fs::File::open(&path).map_err(serde::de::Error::custom)?,
                );

                let pem = rustls_pemfile::read_one(&mut reader)
                    .map_err(serde::de::Error::custom)?
                    .into_iter()
                    .map(|i| match i {
                        rustls_pemfile::Item::RSAKey(i)
                        | rustls_pemfile::Item::X509Certificate(i)
                        | rustls_pemfile::Item::PKCS8Key(i)
                        | rustls_pemfile::Item::ECKey(i) => rustls::PrivateKey(i),
                        _ => todo!(),
                    })
                    .collect::<Vec<_>>();

                pem.first().cloned().ok_or_else(|| {
                    serde::de::Error::custom(format!(
                        "private key path is valid but empty: '{}'",
                        path.display()
                    ))
                })
            } else {
                todo!();
            }
        }
    }

    deserializer.deserialize_any(PrivateKeyVisitor)
}

pub fn serialize<S>(this: &rustls::PrivateKey, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_bytes(&this.0)
}
