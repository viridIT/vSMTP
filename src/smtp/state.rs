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
    Connect,
    Helo,
    NegotiationTLS,
    MailFrom,
    RcptTo,
    Data,
    Stop,
}

impl std::fmt::Display for StateSMTP {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            StateSMTP::Connect => "Connect",
            StateSMTP::Helo => "Helo",
            StateSMTP::NegotiationTLS => "NegotiationTLS",
            StateSMTP::MailFrom => "MailFrom",
            StateSMTP::RcptTo => "RcptTo",
            StateSMTP::Data => "Data",
            StateSMTP::Stop => "Stop",
        })
    }
}

impl From<StateSMTP> for String {
    fn from(state: StateSMTP) -> Self {
        format!("{}", state)
    }
}

#[allow(clippy::module_name_repetitions)]
#[derive(Debug, PartialEq, Eq)]
pub struct StateSMTPFromStrError;

impl std::fmt::Display for StateSMTPFromStrError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("StateSMTPFromStrError")
    }
}

impl std::str::FromStr for StateSMTP {
    type Err = StateSMTPFromStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Connect" => Ok(Self::Connect),
            "Helo" => Ok(Self::Helo),
            "MailFrom" => Ok(Self::MailFrom),
            "NegotiationTLS" => Ok(Self::NegotiationTLS),
            "RcptTo" => Ok(Self::RcptTo),
            "Data" => Ok(Self::Data),
            "Stop" => Ok(Self::Stop),
            _ => Err(StateSMTPFromStrError),
        }
    }
}

impl TryFrom<String> for StateSMTP {
    type Error = StateSMTPFromStrError;

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
            format!("{}", StateSMTP::from_str("root").unwrap_err()),
            "StateSMTPFromStrError"
        );
    }

    #[test]
    fn to_str() {
        assert_eq!(format!("{}", StateSMTP::Connect), "Connect");
        assert_eq!(format!("{}", StateSMTP::Helo), "Helo");
        assert_eq!(format!("{}", StateSMTP::MailFrom), "MailFrom");
        assert_eq!(format!("{}", StateSMTP::RcptTo), "RcptTo");
        assert_eq!(format!("{}", StateSMTP::Data), "Data");
        assert_eq!(format!("{}", StateSMTP::NegotiationTLS), "NegotiationTLS");
        assert_eq!(format!("{}", StateSMTP::Stop), "Stop");
    }

    #[test]
    fn from_str() {
        assert_eq!(StateSMTP::from_str("Connect"), Ok(StateSMTP::Connect));
        assert_eq!(StateSMTP::from_str("Helo"), Ok(StateSMTP::Helo));
        assert_eq!(StateSMTP::from_str("MailFrom"), Ok(StateSMTP::MailFrom));
        assert_eq!(StateSMTP::from_str("RcptTo"), Ok(StateSMTP::RcptTo));
        assert_eq!(StateSMTP::from_str("Data"), Ok(StateSMTP::Data));
        assert_eq!(
            StateSMTP::from_str("NegotiationTLS"),
            Ok(StateSMTP::NegotiationTLS)
        );
        assert_eq!(StateSMTP::from_str("Stop"), Ok(StateSMTP::Stop));
    }

    #[test]
    fn same() {
        for s in <StateSMTP as enum_iterator::IntoEnumIterator>::into_enum_iter() {
            assert_eq!(StateSMTP::from_str(&format!("{}", s)).unwrap(), s);
        }
    }
}
