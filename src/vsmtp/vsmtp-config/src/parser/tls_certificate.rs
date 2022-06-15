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
use vsmtp_common::re::anyhow;

pub fn from_string(input: &str) -> anyhow::Result<rustls::Certificate> {
    let path = std::path::Path::new(&input);
    anyhow::ensure!(
        path.exists(),
        format!("certificate path does not exists: '{}'", path.display())
    );
    let mut reader = std::io::BufReader::new(std::fs::File::open(&path)?);

    let pem = rustls_pemfile::certs(&mut reader)?
        .into_iter()
        .map(rustls::Certificate)
        .collect::<Vec<_>>();

    pem.first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("certificate path is valid but empty: '{}'", path.display()))
}

// TODO: should be used only for debug build
/*
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
            from_string(v).map_err(serde::de::Error::custom)
        }
    }

    deserializer.deserialize_any(CertificateVisitor)
}

#[allow(dead_code)]
pub fn serialize<S>(this: &rustls::Certificate, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let cert = base64::encode(&this.0)
        .chars()
        .collect::<Vec<_>>()
        .chunks(64)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>();

    let mut seq = serializer.serialize_seq(Some(cert.len()))?;
    for i in cert {
        serde::ser::SerializeSeq::serialize_element(&mut seq, &i)?;
    }
    serde::ser::SerializeSeq::end(seq)
}
*/

#[cfg(test)]
mod tests {
    use crate::field::TlsFile;
    use std::io::Write;
    use vsmtp_common::re::serde_json;
    use vsmtp_test::get_tls_file;

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    struct S {
        v: TlsFile<rustls::Certificate>,
    }

    #[test]
    fn basic() {
        let _droppable = std::fs::DirBuilder::new().create("./tmp");

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open("./tmp/crt")
            .unwrap();
        file.write_all(get_tls_file::get_certificate().as_bytes())
            .unwrap();

        serde_json::from_str::<S>(r#"{"v": "./tmp/crt"}"#).unwrap();
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
