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

/// List of supported SASL Mechanism
/// See <https://www.iana.org/assignments/sasl-mechanisms/sasl-mechanisms.xhtml>
#[derive(
    Debug,
    PartialEq,
    Eq,
    Copy,
    Clone,
    Hash,
    PartialOrd,
    Ord,
    strum::EnumIter,
    strum::Display,
    strum::EnumString,
)]
#[strum(serialize_all = "SCREAMING-KEBAB-CASE")]
pub enum Mechanism {
    /// Common, but for interoperability
    Plain,
    /// Obsolete
    Login,
    /// Limited
    CramMd5,
    /// Common
    /// See <https://datatracker.ietf.org/doc/html/rfc4505>
    Anonymous,
    /*
    - EXTERNAL
    - SECURID
    - DIGEST-MD5
    - SCRAM-SHA-1
    - SCRAM-SHA-1-PLUS
    - SCRAM-SHA-256
    - SCRAM-SHA-256-PLUS
    - SAML20
    - OPENID20
    - GSSAPI
    - GS2-KRB5
    - XOAUTH-2
    */
}

impl serde::Serialize for Mechanism {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&format!("{self}"))
    }
}

impl<'de> serde::Deserialize<'de> for Mechanism {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl Default for Mechanism {
    fn default() -> Self {
        // TODO: should it be ?
        Self::Plain
    }
}

impl Mechanism {
    /// Does the client must send data first with initial response
    #[must_use]
    pub const fn client_first(self) -> bool {
        match self {
            Self::Plain | Self::Anonymous => true,
            Self::Login | Self::CramMd5 => false,
        }
    }

    /// Does this mechanism must be under TLS (STARTTLS or Tunnel)
    #[must_use]
    pub const fn must_be_under_tls(self) -> bool {
        match self {
            Self::Plain | Self::Login | Self::CramMd5 | Self::Anonymous => true,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn to_str() {
        assert_eq!(Mechanism::Plain.to_string(), "PLAIN");
        assert_eq!(Mechanism::Login.to_string(), "LOGIN");
        assert_eq!(Mechanism::CramMd5.to_string(), "CRAM-MD5");
        assert_eq!(Mechanism::Anonymous.to_string(), "ANONYMOUS");
    }

    #[test]
    fn serialize() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct S {
            v: Mechanism,
        }

        for i in <Mechanism as strum::IntoEnumIterator>::iter() {
            let s = serde_json::to_string(&S { v: i }).unwrap();
            println!("{s}");
            let s = serde_json::from_str::<S>(&s).unwrap();
            assert_eq!(s.v, i);
        }
    }

    #[test]
    fn supported() {
        let mut rsasl = vsmtp_rsasl::SASL::new_untyped().unwrap();

        let mut supported_by_backend = std::collections::HashMap::new();
        for m in rsasl.server_mech_list().unwrap().iter() {
            println!("{}", m);
            supported_by_backend.insert(
                m.to_string(),
                rsasl.server_supports(&std::ffi::CString::new(m).unwrap()),
            );
        }

        for i in <Mechanism as strum::IntoEnumIterator>::iter() {
            assert!(
                supported_by_backend
                    .get(&format!("{}", i))
                    .unwrap_or(&false),
                "{:?} is declared but not supported",
                i
            );
        }
    }

    #[test]
    fn error() {
        assert!(<Mechanism as std::str::FromStr>::from_str("foobar").is_err());
    }
}
