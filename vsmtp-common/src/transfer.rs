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

/// the delivery method / protocol used for a specific recipient.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Transfer {
    /// relay via the smtp protocol.
    Relay,
    /// local delivery via the mbox protocol.
    Mbox,
    /// local delivery via the maildir protocol.
    Maildir,
    /// the delivery will be skipped.
    None,
}

#[derive(Debug)]
/// errors for the Transfer struct.
pub enum Error {
    /// failed to parse the transfer method from a string.
    FromStr,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::FromStr => write!(f, "failed to parse transfer method from string"),
        }
    }
}

impl Transfer {
    /// return the enum as a static slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Transfer::Relay => "relay",
            Transfer::Mbox => "mbox",
            Transfer::Maildir => "maildir",
            Transfer::None => "none",
        }
    }
}

impl TryFrom<&str> for Transfer {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "relay" => Ok(Self::Relay),
            "mbox" => Ok(Self::Mbox),
            "maildir" => Ok(Self::Maildir),
            "none" => Ok(Self::None),
            _ => Err(Error::FromStr),
        }
    }
}
