/*
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
*/

/// the delivery status of the email of the current rcpt.
// TODO: add timestamp for Sent / HeldBack / Failed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
            EmailTransferStatus::Waiting => "waiting",
            EmailTransferStatus::Sent => "sent",
            EmailTransferStatus::HeldBack(_) => "held back",
            EmailTransferStatus::Failed(_) => "failed",
        }
    }
}

impl std::fmt::Display for EmailTransferStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// the delivery method / protocol used for a specific recipient.
#[derive(Debug, PartialEq, Eq, Hash, Clone, serde::Serialize, serde::Deserialize)]
pub enum Transfer {
    /// forward email via the smtp protocol and mx record resolution.
    Forward(String),
    /// deliver the email via the smtp protocol.
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

impl TryFrom<&str> for Transfer {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "forward" => Ok(Self::Forward(String::default())),
            "deliver" => Ok(Self::Deliver),
            "mbox" => Ok(Self::Mbox),
            "maildir" => Ok(Self::Maildir),
            "none" => Ok(Self::None),
            _ => anyhow::bail!("transfer method '{}' does not exist.", value),
        }
    }
}
