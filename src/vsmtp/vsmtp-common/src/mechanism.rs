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
/// See https://www.iana.org/assignments/sasl-mechanisms/sasl-mechanisms.xhtml
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
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(try_from = "String")]
#[serde(into = "String")]
pub enum Mechanism {
    /// Common, but for interoperability
    Plain,
    /// Obsolete
    Login,
    /// Limited
    CramMd5,
    /*
      ANONYMOUS
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
            Self::Plain => true,
            Self::Login | Self::CramMd5 => false,
        }
    }

    /// Does this mechanism must be under TLS (STARTTLS or Tunnel)
    #[must_use]
    pub const fn must_be_under_tls(self) -> bool {
        match self {
            Self::Plain | Self::Login | Self::CramMd5 => true,
        }
    }
}

impl std::fmt::Display for Mechanism {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Plain => "PLAIN",
            Self::Login => "LOGIN",
            Self::CramMd5 => "CRAM-MD5",
        })
    }
}

impl From<Mechanism> for String {
    fn from(this: Mechanism) -> Self {
        format!("{}", this)
    }
}

impl std::str::FromStr for Mechanism {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PLAIN" => Ok(Self::Plain),
            "LOGIN" => Ok(Self::Login),
            "CRAM-MD5" => Ok(Self::CramMd5),
            _ => anyhow::bail!("not a valid AUTH Mechanism: '{}'", s),
        }
    }
}

impl TryFrom<String> for Mechanism {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        <Self as std::str::FromStr>::from_str(&s)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::str::FromStr;

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
                supported_by_backend.get(&String::from(i)).unwrap_or(&false),
                "{:?} is declared but not supported",
                i
            );
        }
    }

    #[test]
    fn error() {
        assert_eq!(
            format!("{}", Mechanism::from_str("foobar").unwrap_err()),
            "not a valid AUTH Mechanism: 'foobar'"
        );
    }

    #[test]
    fn same() {
        for s in <Mechanism as strum::IntoEnumIterator>::iter() {
            println!("{:?}", s);
            assert_eq!(Mechanism::from_str(&format!("{}", s)).unwrap(), s);
            assert_eq!(String::try_from(s).unwrap(), format!("{}", s));
            let str: String = s.into();
            assert_eq!(str, format!("{}", s));
        }
    }
}
