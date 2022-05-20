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

use std::fmt::Display;

/// A packet send from the application (.vsl) to the server (vsmtp)
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum InfoPacket {
    /// a string
    Str(String),
    /// a custom code.
    Code {
        /// the base code (550, 250 ...)
        base: i64,
        /// the enhanced code {5.7.1 ...}
        enhanced: String,
        /// a message to send.
        text: String,
    },
}

impl Display for InfoPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                InfoPacket::Str(string) => string.clone(),
                InfoPacket::Code {
                    base,
                    enhanced,
                    text,
                } => {
                    format!("{base} {enhanced} {text}")
                }
            }
        )
    }
}

/// Status of the mail context treated by the rule engine
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Status {
    /// informational data needs to be sent to the client.
    Info(InfoPacket),

    /// accepts the current stage value, skips all rules in the stage.
    Accept,

    /// continue to the next rule / stage.
    Next,

    /// immediately stops the transaction and send an error code.
    Deny(Option<InfoPacket>),

    /// ignore all future rules for the current transaction.
    Faccept,

    /// ignore all future rules for the current transaction.
    /// the String parameter is the path to the quarantine folder.
    Quarantine(String),
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Info(_) => "info",
                Self::Accept => "accept",
                Self::Next => "next",
                Self::Deny(_) => "deny",
                Self::Faccept => "faccept",
                Self::Quarantine(_) => "quarantine",
            }
        )
    }
}

#[cfg(test)]
mod test {
    use crate::status::Status;

    use super::InfoPacket;

    #[test]
    fn display_status() {
        println!(
            "{}, {}, {}, {}, {}, {}",
            Status::Info(InfoPacket::Str(String::default())),
            Status::Accept,
            Status::Next,
            Status::Deny(None),
            Status::Faccept,
            Status::Quarantine(String::default()),
        );
    }

    #[test]
    fn to_string() {
        assert_eq!(
            InfoPacket::Str("packet".to_string()).to_string().as_str(),
            "packet"
        );

        assert_eq!(
            InfoPacket::Code {
                base: 250,
                enhanced: "2.0.0".to_string(),
                text: "custom message".to_string()
            }
            .to_string()
            .as_str(),
            "250 2.0.0 custom message"
        );
    }
}
