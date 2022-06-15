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
/// Address Email
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Eq)]
#[serde(into = "String", try_from = "String")]
pub struct Address {
    #[serde(skip)]
    at_sign: usize,
    full: String,
}

/// Syntax sugar Address object from dyn `ToString`
///
/// # Panics
///
/// if the argument failed to be converted
#[macro_export]
macro_rules! addr {
    ($e:expr) => {
        $crate::Address::try_from($e.to_string()).unwrap()
    };
}

impl TryFrom<String> for Address {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Err(error) = addr::parse_email_address(&value) {
            anyhow::bail!("'{}' is not a valid address: {}", value, error)
        }
        Ok(Self {
            at_sign: value.find('@').expect("no '@' in address"),
            full: value,
        })
    }
}

impl From<Address> for String {
    fn from(value: Address) -> Self {
        value.full().to_string()
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

    /// create a new address without verifying the syntax.
    ///
    /// # Panics
    ///
    /// * there is no '@' characters in the string
    #[must_use]
    pub fn new_unchecked(addr: String) -> Self {
        Self {
            at_sign: addr.find('@').unwrap(),
            full: addr,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn deserialize() {
        let parsed = serde_json::from_str::<Address>(r#""hello@domain.com""#).unwrap();
        assert_eq!(
            parsed,
            Address {
                full: "hello@domain.com".to_string(),
                at_sign: 6
            }
        );
        assert_eq!(parsed.local_part(), "hello");
        assert_eq!(parsed.domain(), "domain.com");
    }

    #[test]
    fn serialize() {
        assert_eq!(
            serde_json::to_string(&Address {
                full: "hello@domain.com".to_string(),
                at_sign: 6
            })
            .unwrap(),
            r#""hello@domain.com""#
        );
    }
}
