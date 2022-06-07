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

use crate::ReplyOrCodeID;

/// Status of the mail context treated by the rule engine
#[derive(Debug, Clone, PartialEq, Eq, strum::AsRefStr, serde::Deserialize, serde::Serialize)]
#[strum(serialize_all = "snake_case")]
pub enum Status {
    /// informational data needs to be sent to the client.
    Info(ReplyOrCodeID),

    /// accepts the current [`crate::state::StateSMTP`] value, skips all rules in the stage.
    Accept(ReplyOrCodeID),

    /// continue to the next rule / stage.
    Next,

    /// immediately stops the transaction and send an error code.
    Deny(ReplyOrCodeID),

    /// ignore all future rules for the transaction.
    Faccept(ReplyOrCodeID),

    /// ignore all future rules for the transaction.
    /// the String parameter is the path to the quarantine folder.
    /// this status disable delivery to all recipients.
    Quarantine(String),

    /// used to send data from .vsl to vsmtp's server
    Packet(String),
}
