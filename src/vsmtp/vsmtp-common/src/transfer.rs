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

/// Error produced received by the Queue manager
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq, strum::Display, serde::Serialize, serde::Deserialize)]
pub enum TransferErrors {
    /// For local delivery (Maildir / Mbox), the requested mailbox does not exist on the system
    NoSuchMailbox {
        /// Name requested
        name: String,
    },

    /// TODO: used for convenience, should be removed
    Other(String),
}

impl From<anyhow::Error> for TransferErrors {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<String> for TransferErrors {
    fn from(e: String) -> Self {
        Self::Other(e)
    }
}

/// the delivery status of the email of the current rcpt.
#[derive(Debug, Clone, Eq, strum::Display, serde::Serialize, serde::Deserialize)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum EmailTransferStatus {
    /// the email has not been sent yet.
    /// the email is in the deliver / working queue at this point.
    Waiting {
        /// timestamp when the status has been set
        timestamp: std::time::SystemTime,
    },
    /// email for this recipient has been successfully sent.
    /// When all [`crate::rcpt::Rcpt`] are [`EmailTransferStatus::Sent`], the files are removed from disk.
    Sent {
        /// timestamp when the status has been set
        timestamp: std::time::SystemTime,
    },
    /// the delivery failed, the system is trying to re-send the email.
    /// the email is located in the deferred queue at this point.
    HeldBack {
        /// timestamp when the status has been set
        errors: Vec<(std::time::SystemTime, TransferErrors)>,
    },
    /// the email failed too many times. the argument is the reason of the failure.
    /// the email is probably written in the dead or quarantine queues at this point.
    Failed {
        /// timestamp when the status has been set
        timestamp: std::time::SystemTime,
        /// why it is considered failed
        reason: String,
    },
}

impl PartialEq<EmailTransferStatus> for EmailTransferStatus {
    fn eq(&self, other: &EmailTransferStatus) -> bool {
        match (self, other) {
            (Self::Sent { .. }, Self::Sent { .. })
            | (Self::Waiting { .. }, Self::Waiting { .. }) => true,
            (Self::HeldBack { errors: l_errors }, Self::HeldBack { errors: r_errors }) => l_errors
                .iter()
                .map(|(_, errors)| errors)
                .eq(r_errors.iter().map(|(_, errors)| errors)),
            (
                Self::Failed {
                    reason: l_reason, ..
                },
                Self::Failed {
                    reason: r_reason, ..
                },
            ) => l_reason == r_reason,
            _ => false,
        }
    }
}

impl EmailTransferStatus {
    /// Should the recipient be delivered, or it has been done already ?
    #[must_use]
    pub const fn is_sendable(&self) -> bool {
        match self {
            EmailTransferStatus::Waiting { .. } | EmailTransferStatus::HeldBack { .. } => true,
            EmailTransferStatus::Sent { .. } | EmailTransferStatus::Failed { .. } => false,
        }
    }

    /// Set the status to [`EmailTransferStatus::HeldBack`] with an error, or increase the previous stack.
    pub fn held_back(&mut self, error: impl Into<TransferErrors>) {
        let error = error.into();
        match self {
            EmailTransferStatus::HeldBack { errors } => {
                errors.push((std::time::SystemTime::now(), error));
            }
            _ => {
                *self = Self::HeldBack {
                    errors: vec![(std::time::SystemTime::now(), error)],
                }
            }
        }
    }
}

/// possible format of the forward target.
#[derive(
    Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, serde::Serialize, serde::Deserialize,
)]
pub enum ForwardTarget {
    /// the target is a domain name. (default)
    Domain(String),
    /// the target is an ip address, a domain resolution needs to be made.
    Ip(std::net::IpAddr),
    /// the target is an ip address with an associated port.
    Socket(std::net::SocketAddr),
}

/// the delivery method / protocol used for a specific recipient.
#[derive(
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Clone,
    strum::Display,
    serde::Serialize,
    serde::Deserialize,
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
    /// it to the correct type.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.find('%').map_or_else(
            || {
                s.parse::<std::net::SocketAddr>().map_or_else(
                    |_| {
                        s.parse::<std::net::IpAddr>().map_or_else(
                            |_| {
                                addr::parse_domain_name(s)
                                    .map(|domain| ForwardTarget::Domain(domain.to_string()))
                                    .map_err(|err| {
                                        anyhow::anyhow!(
                                            "{} could not be used as a forward target.",
                                            err.input()
                                        )
                                    })
                            },
                            |ip| Ok(ForwardTarget::Ip(ip)),
                        )
                    },
                    |socket| Ok(ForwardTarget::Socket(socket)),
                )
            },
            |_| -> Result<ForwardTarget, _> { ipv6_with_scope_id(s).map(ForwardTarget::Socket) },
        )
    }
}

/// a transport using the smtp protocol.
/// (mostly a new type over `lettre::SmtpTransport` to implement debug
/// and make switching transport easy if needed)
#[derive(Clone)]
pub struct SmtpConnection(pub std::sync::Arc<std::sync::Mutex<lettre::SmtpTransport>>);

impl Eq for SmtpConnection {}
impl PartialEq for SmtpConnection {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

impl std::fmt::Debug for SmtpConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SmtpTransport").finish()
    }
}
