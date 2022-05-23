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
/// the delivery status of the email of the current rcpt.
// TODO: add timestamp for Sent / HeldBack / Failed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

impl EmailTransferStatus {
    /// get the associated string slice of the variant.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Sent => "sent",
            Self::HeldBack(_) => "held back",
            Self::Failed(_) => "failed",
        }
    }
}

impl std::fmt::Display for EmailTransferStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// possible format of the forward target.
#[derive(Debug, PartialEq, Eq, Hash, Clone, serde::Serialize, serde::Deserialize)]
pub enum ForwardTarget {
    /// the target is a domain name. (default)
    Domain(String),
    /// the target is an ip address, a domaine resolution needs to be made.
    Ip(std::net::IpAddr),
}

/// the delivery method / protocol used for a specific recipient.
#[derive(Debug, PartialEq, Eq, Hash, Clone, serde::Serialize, serde::Deserialize)]
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

impl Transfer {
    /// return the enum as a static slice.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Forward(..) => "forward",
            Self::Deliver => "deliver",
            Self::Mbox => "mbox",
            Self::Maildir => "maildir",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for Transfer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Transfer {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "forward" => Ok(Self::Forward(ForwardTarget::Domain(String::default()))),
            "deliver" => Ok(Self::Deliver),
            "mbox" => Ok(Self::Mbox),
            "maildir" => Ok(Self::Maildir),
            "none" => Ok(Self::None),
            _ => anyhow::bail!("transfer method '{}' does not exist.", s),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::{EmailTransferStatus, Transfer};

    mod status {
        use super::EmailTransferStatus;

        #[test]
        fn display() {
            for i in [
                EmailTransferStatus::Waiting,
                EmailTransferStatus::Sent,
                EmailTransferStatus::HeldBack(usize::default()),
                EmailTransferStatus::Failed(String::default()),
            ] {
                println!("{}", i);
            }
        }
    }

    mod transfer {
        use crate::transfer::ForwardTarget;

        use super::Transfer;
        use std::str::FromStr;

        #[test]
        fn error() {
            assert_eq!(
                format!("{}", Transfer::from_str("foobar").unwrap_err()),
                "transfer method 'foobar' does not exist."
            );
        }

        #[test]
        fn same() {
            for s in [
                Transfer::None,
                Transfer::Deliver,
                Transfer::Maildir,
                Transfer::Mbox,
                Transfer::Forward(ForwardTarget::Domain(String::default())),
            ] {
                println!("{:?}", s);
                assert_eq!(Transfer::from_str(&format!("{}", s)).unwrap(), s);
            }
        }
    }
}
