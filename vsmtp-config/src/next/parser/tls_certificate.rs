pub fn deserialize<'de, D>(deserializer: D) -> Result<rustls::Certificate, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct CertificateVisitor;

    impl<'de> serde::de::Visitor<'de> for CertificateVisitor {
        type Value = rustls::Certificate;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("[...]")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let path = std::path::Path::new(&v);
            if path.exists() {
                let mut reader = std::io::BufReader::new(
                    std::fs::File::open(&path).map_err(serde::de::Error::custom)?,
                );

                let pem = rustls_pemfile::certs(&mut reader)
                    .map_err(serde::de::Error::custom)?
                    .into_iter()
                    .map(rustls::Certificate)
                    .collect::<Vec<_>>();

                pem.first().cloned().ok_or_else(|| {
                    serde::de::Error::custom(format!(
                        "certificate path is valid but empty: '{}'",
                        path.display()
                    ))
                })
            } else {
                let mut cursor = std::io::Cursor::new(v);
                let pem = rustls_pemfile::certs(&mut cursor)
                    .map_err(serde::de::Error::custom)?
                    .into_iter()
                    .map(rustls::Certificate)
                    .collect::<Vec<_>>();

                pem.first().cloned().ok_or_else(|| {
                    serde::de::Error::custom(format!(
                        "certificate is invalid: '{}'",
                        path.display()
                    ))
                })
            }
        }
    }

    deserializer.deserialize_any(CertificateVisitor)
}

pub fn serialize<S>(this: &rustls::Certificate, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_bytes(&this.0)
}
