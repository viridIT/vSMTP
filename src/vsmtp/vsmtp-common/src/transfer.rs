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

use crate::utils::ipv6_with_scope_id;

// TODO: add timestamp for Sent / HeldBack / Failed.
/// the delivery status of the email of the current rcpt.
#[derive(Debug, Clone, PartialEq, Eq, strum::Display, serde::Serialize, serde::Deserialize)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum EmailTransferStatus {
    /// the email has not been sent yet.
    /// the email is in the deliver / working queue at this point.
    Waiting,
    /// email for this recipient has been successfully sent.
    /// the email has been removed from all queues at this point.
    Sent,
    /// the delivery failed, the system is trying to re-send the email.
    /// the email is located in the deferred queue at this point.
    /// TODO: add error on deferred.
    HeldBack(usize),
    /// the email failed to be sent. the argument is the reason of the failure.
    /// the email is probably written in the dead or quarantine queues at this point.
    Failed(String),
    // NOTE: is Quarantined(String) useful, or we just use Failed(String) instead ?
}

/// possible format of the forward target.
#[derive(Debug, PartialEq, Eq, Hash, Clone, serde::Serialize, serde::Deserialize)]
pub enum ForwardTarget {
    /// the target is a domain name. (default)
    Domain(String),
    /// the target is an ip address, a domaine resolution needs to be made.
    Ip(std::net::IpAddr),
    /// the target is an ip address with an associated port.
    Socket(std::net::SocketAddr),
}

/// the delivery method / protocol used for a specific recipient.
#[derive(
    Debug, PartialEq, Eq, Hash, Clone, strum::Display, serde::Serialize, serde::Deserialize,
)]
#[strum(serialize_all = "snake_case")]
pub enum Transfer {
    /// forward email via the smtp protocol.
    Forward(ForwardTarget),
    /// deliver the email via the smtp protocol and mx record resolution.
    Deliver,
    /// local delivery via the mbox protocol.
    Mbox,
    /// local delivery via the maildir protocol.
    Maildir,
    /// the delivery will be skipped.
    None,
}

impl std::str::FromStr for ForwardTarget {
    type Err = anyhow::Error;

    /// create a forward target from a string and cast
    /// it to the corect type.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.find('%').map_or_else(
            || {
                s.parse::<std::net::SocketAddr>().map_or_else(
                    |_| {
                        Ok(s.parse::<std::net::IpAddr>().map_or_else(
                            |_| ForwardTarget::Domain(s.to_string()),
                            ForwardTarget::Ip,
                        ))
                    },
                    |socket| Ok(ForwardTarget::Socket(socket)),
                )
            },
            |_| -> Result<ForwardTarget, _> { ipv6_with_scope_id(s).map(ForwardTarget::Socket) },
        )
    }
}
