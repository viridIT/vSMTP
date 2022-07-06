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
use crate::{
    transfer::{EmailTransferStatus, Transfer},
    Address,
};

/// representation of a recipient with it's delivery method.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Rcpt {
    /// email address of the recipient.
    pub address: Address,
    /// protocol used by vsmtp to deliver / transfer the email bound by this recipient.
    pub transfer_method: Transfer,
    /// delivery status of the email bound to this recipient.
    pub email_status: EmailTransferStatus,
}

impl Rcpt {
    /// create a new recipient from it's address.
    /// there is no transfer method by default.
    #[must_use]
    pub fn new(address: Address) -> Self {
        Self {
            address,
            transfer_method: Transfer::Deliver,
            email_status: EmailTransferStatus::Waiting {
                timestamp: std::time::SystemTime::now(),
            },
        }
    }

    /// create a new recipient from it's address & transfer method.
    #[must_use]
    pub fn with_transfer_method(address: Address, method: Transfer) -> Self {
        Self {
            address,
            transfer_method: method,
            email_status: EmailTransferStatus::Waiting {
                timestamp: std::time::SystemTime::now(),
            },
        }
    }
}

impl From<Address> for Rcpt {
    fn from(this: Address) -> Self {
        Self::new(this)
    }
}

impl std::fmt::Display for Rcpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.address)
    }
}

impl PartialEq<Rcpt> for Address {
    fn eq(&self, other: &Rcpt) -> bool {
        *self == other.address
    }
}
