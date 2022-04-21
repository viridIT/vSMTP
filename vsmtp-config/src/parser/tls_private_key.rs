use vsmtp_common::re::{anyhow, base64};

pub fn from_string(input: &str) -> anyhow::Result<rustls::PrivateKey> {
    let path = std::path::Path::new(input);
    anyhow::ensure!(
        path.exists(),
        format!("private key path does not exists: '{}'", path.display())
    );
    let mut reader = std::io::BufReader::new(std::fs::File::open(&path)?);

    let pem = rustls_pemfile::read_one(&mut reader)?
        .into_iter()
        .map(|i| match i {
            rustls_pemfile::Item::RSAKey(i)
            | rustls_pemfile::Item::PKCS8Key(i)
            | rustls_pemfile::Item::ECKey(i) => Ok(rustls::PrivateKey(i)),
            _ => Err(anyhow::anyhow!("private key is valid but not supported")),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    pem.first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("private key path is valid but empty: '{}'", path.display()))
}

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
            from_string(v).map_err(serde::de::Error::custom)
        }
    }

    deserializer.deserialize_any(PrivateKeyVisitor)
}

#[allow(dead_code)]
pub fn serialize<S>(this: &rustls::PrivateKey, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let key = base64::encode(&this.0)
        .chars()
        .collect::<Vec<_>>()
        .chunks(64)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>();

    let mut seq = serializer.serialize_seq(Some(key.len()))?;
    for i in key {
        serde::ser::SerializeSeq::serialize_element(&mut seq, &i)?;
    }
    serde::ser::SerializeSeq::end(seq)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use vsmtp_common::re::serde_json;
    use vsmtp_test::get_tls_file;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct S {
        #[serde(
            serialize_with = "crate::parser::tls_private_key::serialize",
            deserialize_with = "crate::parser::tls_private_key::deserialize"
        )]
        v: rustls::PrivateKey,
    }

    #[test]
    fn rsa_ok() {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open("./tmp/rsa_key")
            .unwrap();
        file.write_all(get_tls_file::get_rsa_key().as_bytes())
            .unwrap();

        serde_json::from_str::<S>(r#"{"v": "./tmp/rsa_key"}"#).unwrap();
    }

    #[test]
    fn pkcs8_ok() {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open("./tmp/pkcs8_key")
            .unwrap();
        file.write_all(get_tls_file::get_pkcs8_key().as_bytes())
            .unwrap();

        serde_json::from_str::<S>(r#"{"v": "./tmp/pkcs8_key"}"#).unwrap();
    }

    #[test]
    fn ec256_ok() {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open("./tmp/ec256_key")
            .unwrap();
        file.write_all(get_tls_file::get_ec256_key().as_bytes())
            .unwrap();

        serde_json::from_str::<S>(r#"{"v": "./tmp/ec256_key"}"#).unwrap();
    }

    #[test]
    fn not_good_format() {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open("./tmp/crt2")
            .unwrap();
        file.write_all(get_tls_file::get_certificate().as_bytes())
            .unwrap();

        serde_json::from_str::<S>(r#"{"v": "./tmp/crt2"}"#).unwrap_err();
    }

    #[test]
    fn not_a_string() {
        serde_json::from_str::<S>(r#"{"v": 10}"#).unwrap_err();
    }

    #[test]
    fn not_valid_path() {
        serde_json::from_str::<S>(r#"{"v": "foobar"}"#).unwrap_err();
    }
}
