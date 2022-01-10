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
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone, serde::Deserialize, serde::Serialize)]
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

pub struct StateSMTPFromStrError;

impl std::fmt::Display for StateSMTPFromStrError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("SourceFromStrError")
    }
}

impl std::str::FromStr for StateSMTP {
    type Err = StateSMTPFromStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Connect" => Ok(StateSMTP::Connect),
            "Helo" => Ok(StateSMTP::Helo),
            "MailFrom" => Ok(StateSMTP::MailFrom),
            "RcptTo" => Ok(StateSMTP::RcptTo),
            "Data" => Ok(StateSMTP::Data),
            _ => Err(StateSMTPFromStrError),
        }
    }
}
