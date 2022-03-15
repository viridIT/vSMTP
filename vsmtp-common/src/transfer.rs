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

/// the delivery status of the email of the current rcpt.
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
    HeldBack(usize),
    /// the email failed to be sent. the argument is the reason of the failure.
    /// the email is probably written in the dead or quarantine queues at this point.
    Failed(String),
}

/// the delivery method / protocol used for a specific recipient.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum Transfer {
    /// relay via the smtp protocol.
    Relay,
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
            Transfer::Relay => "relay",
            Transfer::Mbox => "mbox",
            Transfer::Maildir => "maildir",
            Transfer::None => "none",
        }
    }
}

impl TryFrom<&str> for Transfer {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "relay" => Ok(Self::Relay),
            "mbox" => Ok(Self::Mbox),
            "maildir" => Ok(Self::Maildir),
            "none" => Ok(Self::None),
            _ => anyhow::bail!("transfer method '{}' does not exist.", value),
        }
    }
}
