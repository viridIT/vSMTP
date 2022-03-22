/// List of supported SASL Mechanism
#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash, PartialOrd, Ord, strum::EnumIter)]
pub enum Mechanism {
    /// For interoperability
    Plain,
    // Login,
    ///
    Gssapi,
}

impl Default for Mechanism {
    fn default() -> Self {
        // TODO:
        Self::Plain
    }
}

impl Mechanism {
    /// Does the client must send data first with initial response
    #[must_use]
    pub const fn client_first(self) -> bool {
        match self {
            Mechanism::Plain => true,
            Mechanism::Gssapi => true,
            // Mechanism::Login => todo!(),
        }
    }

    /// Does this mechanism must be under TLS (STARTTLS or Tunnel)
    #[must_use]
    pub const fn must_be_under_tls(self) -> bool {
        match self {
            Mechanism::Plain => true,
            Mechanism::Gssapi => true,
            // Mechanism::Login => todo!(),
        }
    }
}

impl From<Mechanism> for String {
    fn from(this: Mechanism) -> Self {
        match this {
            Mechanism::Plain => "PLAIN",
            Mechanism::Gssapi => "GSSAPI",
            // Mechanism::Login => "LOGIN",
        }
        .to_string()
    }
}

impl std::str::FromStr for Mechanism {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "PLAIN" => Ok(Self::Plain),
            "GSSAPI" => Ok(Self::Gssapi),
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
}
