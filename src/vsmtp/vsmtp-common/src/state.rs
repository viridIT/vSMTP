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
use crate::mechanism::Mechanism;

/// State of the pipeline SMTP
#[derive(
    Debug,
    Eq,
    PartialEq,
    Hash,
    Clone,
    Ord,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
    strum::EnumIter,
)]
#[serde(untagged)]
#[serde(into = "String")]
#[serde(try_from = "String")]
#[allow(clippy::module_name_repetitions)]
pub enum StateSMTP {
    /// After TCP/IP socket has been accepted
    Connect,
    /// After receiving HELO/EHLO command
    Helo,
    /// After receiving STARTTLS command
    NegotiationTLS,
    /// After receiving AUTH command
    Authentication(Mechanism, Option<Vec<u8>>),
    /// After receiving MAIL FROM command
    MailFrom,
    /// After receiving RCPT TO command
    RcptTo,
    /// After receiving DATA command
    Data,
    /// Before write on disk
    PreQ,
    /// After receiving QUIT command
    Stop,
    /// After connection closed
    PostQ,
    /// Right before sending to recipient
    Delivery,
}

impl Default for StateSMTP {
    fn default() -> Self {
        Self::Connect
    }
}

impl std::fmt::Display for StateSMTP {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            // format used by vSL
            Self::Connect => "connect",
            Self::Helo => "helo",
            Self::MailFrom => "mail",
            Self::RcptTo => "rcpt",
            Self::PreQ => "preq",
            Self::PostQ => "postq",
            Self::Delivery => "delivery",
            Self::Authentication(_, _) => "authenticate",
            Self::Data => "data",
            // others
            Self::Stop => "Stop",
            Self::NegotiationTLS => "NegotiationTLS",
        })
    }
}

impl From<StateSMTP> for String {
    fn from(state: StateSMTP) -> Self {
        format!("{}", state)
    }
}

impl std::str::FromStr for StateSMTP {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            // format used by vSL
            "connect" => Ok(Self::Connect),
            "helo" => Ok(Self::Helo),
            "mail" => Ok(Self::MailFrom),
            "rcpt" => Ok(Self::RcptTo),
            "preq" => Ok(Self::PreQ),
            "postq" => Ok(Self::PostQ),
            "delivery" => Ok(Self::Delivery),
            "authenticate" => Ok(Self::Authentication(
                Mechanism::default(),
                Option::<Vec<u8>>::default(),
            )),
            "data" => Ok(Self::Data),
            // others
            "Stop" => Ok(Self::Stop),
            "NegotiationTLS" => Ok(Self::NegotiationTLS),
            _ => anyhow::bail!("not a valid SMTP state: '{}'", s),
        }
    }
}

impl TryFrom<String> for StateSMTP {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        <Self as std::str::FromStr>::from_str(&value)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::StateSMTP;

    #[test]
    fn error() {
        assert_eq!(
            format!("{}", StateSMTP::from_str("foobar").unwrap_err()),
            "not a valid SMTP state: 'foobar'"
        );
    }

    #[test]
    fn same() {
        for s in <StateSMTP as strum::IntoEnumIterator>::iter() {
            println!("{:?}", s);
            assert_eq!(StateSMTP::from_str(&format!("{}", s)).unwrap(), s);
            assert_eq!(String::try_from(s.clone()).unwrap(), format!("{}", s));
            let str: String = s.clone().into();
            assert_eq!(str, format!("{}", s));
        }
    }
}
