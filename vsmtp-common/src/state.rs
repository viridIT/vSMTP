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

/// State of the pipeline SMTP
#[derive(
    Debug,
    Eq,
    PartialEq,
    Hash,
    Copy,
    Clone,
    Ord,
    PartialOrd,
    enum_iterator::IntoEnumIterator,
    serde::Deserialize,
    serde::Serialize,
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

impl std::fmt::Display for StateSMTP {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            // format used by vSL
            StateSMTP::Connect => "connect",
            StateSMTP::Helo => "helo",
            StateSMTP::MailFrom => "mail",
            StateSMTP::RcptTo => "rcpt",
            StateSMTP::PreQ => "preq",
            StateSMTP::PostQ => "postq",
            StateSMTP::Delivery => "delivery",
            // others
            StateSMTP::Data => "Data",
            StateSMTP::Stop => "Stop",
            StateSMTP::NegotiationTLS => "NegotiationTLS",
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
            // others
            "Data" => Ok(Self::Data),
            "Stop" => Ok(Self::Stop),
            "NegotiationTLS" => Ok(Self::NegotiationTLS),
            _ => Err(anyhow::anyhow!("not a valid SMTP state: '{}'", s)),
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
        for s in <StateSMTP as enum_iterator::IntoEnumIterator>::into_enum_iter() {
            println!("{:?}", s);
            assert_eq!(StateSMTP::from_str(&format!("{}", s)).unwrap(), s);
            assert_eq!(String::try_from(s).unwrap(), format!("{}", s));
        }
    }
}
