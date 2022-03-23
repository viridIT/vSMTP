/// List of supported SASL Mechanism
#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash, PartialOrd, Ord, strum::EnumIter)]
pub enum Mechanism {
    /// For interoperability
    Plain,
    ///
    Login,
    /*
      ANONYMOUS
    - EXTERNAL
    - SECURID
    - DIGEST-MD5
    - CRAM-MD5
    - SCRAM-SHA-1
    - SCRAM-SHA-1-PLUS
    - SCRAM-SHA-256
    - SCRAM-SHA-256-PLUS
    - SAML20
    - OPENID20
    - GSSAPI
    - GS2-KRB5
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
            Mechanism::Plain => true,
            Mechanism::Login => false,
        }
    }

    /// Does this mechanism must be under TLS (STARTTLS or Tunnel)
    #[must_use]
    pub const fn must_be_under_tls(self) -> bool {
        match self {
            Mechanism::Plain | Mechanism::Login => true,
        }
    }
}

impl std::fmt::Display for Mechanism {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Mechanism::Plain => "PLAIN",
            Mechanism::Login => "LOGIN",
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
            // "GSSAPI" => Ok(Self::Gssapi),
            "LOGIN" => Ok(Self::Login),
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
        let mut rsasl = rsasl::SASL::new_untyped().unwrap();

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
            "not a valid SMTP state: 'foobar'"
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
