/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub(crate) struct AddressParsingError(String);

impl Display for AddressParsingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for AddressParsingError {}
impl From<&str> for AddressParsingError {
    fn from(s: &str) -> Self {
        Self { 0: s.to_string() }
    }
}

/// using a custom struct for addresses instead of addr::email::Address
/// because addr::email::Address contains a lifetime parameter.
/// since addr::email::Address needs to be sent in rhai's context,
/// it needs to be static, thus impossible to do.
/// TODO: find a way to use addr::email::Address instead of this struct.
#[derive(Clone, Debug)]
pub(crate) struct Address {
    full: String,
    at_sign: usize,
}

impl Address {
    pub(crate) fn new(addr: &str) -> Result<Self, AddressParsingError> {
        match addr::parse_email_address(addr) {
            Ok(addr) => Ok(Self {
                full: addr.to_string(),
                at_sign: addr
                    .as_str()
                    .find("@")
                    .ok_or_else::<AddressParsingError, _>(|| "Failed to parse address".into())?,
            }),
            Err(error) => Err(format!("'{}' is not a valid address: {}", addr, error)
                .as_str()
                .into()),
        }
    }

    /// get the full email address.
    pub(crate) fn full(&self) -> &str {
        &self.full
    }

    /// get the user of the address.
    pub(crate) fn user(&self) -> &str {
        &self.full[..self.at_sign]
    }

    /// get the fqdn of the address.
    pub(crate) fn domain(&self) -> &str {
        &self.full[self.at_sign..]
    }
}
