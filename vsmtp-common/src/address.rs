/**
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 *  This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
**/
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct AddressParsingError(String);

impl std::fmt::Display for AddressParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for AddressParsingError {}

impl From<&str> for AddressParsingError {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// using a custom struct for addresses instead of addr::email::Address
/// because addr::email::Address contains a lifetime parameter.
/// since addr::email::Address needs to be sent in rhai's context,
/// it needs to be static, thus impossible to do.
/// TODO: find a way to use addr::email::Address instead of this struct.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Eq)]
pub struct Address {
    full: String,
    // TODO: ignore serialize ?
    at_sign: usize,
}

impl Default for Address {
    // NOTE: this object shouldn't expose the default trait,
    //       this is just for convenience for now, but it will
    //       need to be removed later.
    fn default() -> Self {
        Self {
            full: "default@address.com".to_string(),
            at_sign: 7,
        }
    }
}

impl PartialEq for Address {
    fn eq(&self, other: &Self) -> bool {
        self.full == other.full
    }
}

impl std::hash::Hash for Address {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.full.hash(state);
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full)
    }
}

impl Address {
    /// Create a new address from a string, fail is invalid
    ///
    /// # Errors
    ///
    /// * addr is not rfc compliant
    pub fn new(addr: &str) -> Result<Self, AddressParsingError> {
        match addr::parse_email_address(addr) {
            Ok(addr) => Ok(Self {
                full: addr.to_string(),
                at_sign: addr
                    .as_str()
                    .find('@')
                    .ok_or_else::<AddressParsingError, _>(|| "Failed to parse address".into())?,
            }),
            Err(error) => Err(format!("'{}' is not a valid address: {}", addr, error)
                .as_str()
                .into()),
        }
    }

    /// get the full email address.
    #[must_use]
    pub fn full(&self) -> &str {
        &self.full
    }

    /// get the user of the address.
    #[must_use]
    pub fn local_part(&self) -> &str {
        &self.full[..self.at_sign]
    }

    /// get the fqdn of the address.
    #[must_use]
    pub fn domain(&self) -> &str {
        &self.full[self.at_sign + 1..]
    }
}

impl From<crate::rcpt::Rcpt> for Address {
    fn from(rcpt: crate::rcpt::Rcpt) -> Self {
        rcpt.address
    }
}

#[allow(clippy::from_over_into)]
impl Into<crate::rcpt::Rcpt> for Address {
    fn into(self) -> crate::rcpt::Rcpt {
        crate::rcpt::Rcpt::new(self)
    }
}
