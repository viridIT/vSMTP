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

use crate::{Reply, ReplyOrCodeID};

/// A packet send from the application (.vsl) to the server (vsmtp)
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum InfoPacket {
    /// a string
    Str(String),
    /// a custom code.
    Code(Reply),
}

impl std::fmt::Display for InfoPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InfoPacket::Str(string) => write!(f, "{string}"),
            InfoPacket::Code(reply) => write!(f, "{} {}", reply.code(), reply.text()),
        }
    }
}

/// Status of the mail context treated by the rule engine
#[derive(Debug, Clone, PartialEq, Eq, strum::AsRefStr, serde::Deserialize, serde::Serialize)]
#[strum(serialize_all = "snake_case")]
pub enum Status {
    /// informational data needs to be sent to the client.
    Info(InfoPacket),

    /// accepts the current stage value, skips all rules in the stage.
    Accept,

    /// continue to the next rule / stage.
    Next,

    /// immediately stops the transaction and send an error code.
    Deny(ReplyOrCodeID),

    /// ignore all future rules for the current transaction.
    Faccept,

    /// ignore all future rules for the current transaction.
    /// the String parameter is the path to the quarantine folder.
    Quarantine(String),
}

#[cfg(test)]
mod test {
    use crate::{Reply, ReplyCode};

    use super::InfoPacket;

    #[test]
    fn to_string() {
        assert_eq!(
            InfoPacket::Str("packet".to_string()).to_string().as_str(),
            "packet"
        );

        assert_eq!(
            InfoPacket::Code(Reply::new(
                ReplyCode::Enhanced {
                    code: 250,
                    enhanced: "2.0.0".to_string(),
                },
                "custom message".to_string()
            ))
            .to_string()
            .as_str(),
            "250 2.0.0 custom message"
        );
    }
}
