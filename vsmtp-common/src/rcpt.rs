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
use crate::{
    address::Address,
    transfer::{EmailTransferStatus, Transfer},
};

/// representation of a recipient with it's delivery method.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Rcpt {
    /// email address of the recipient.
    pub address: Address,
    /// protocol used by vsmtp to deliver / transfer the email bound by this recipient.
    pub transfer_method: Transfer,
    /// delivery status of the email bound to this recipient.
    pub email_status: EmailTransferStatus,
    /// number of times the mta tried to send an email for this rcpt.
    pub retry: usize,
}

impl Rcpt {
    /// create a new recipient from it's address.
    /// the delivery method is set tp default.
    #[must_use]
    pub const fn new(address: Address) -> Self {
        Self {
            address,
            transfer_method: Transfer::None,
            email_status: EmailTransferStatus::Waiting,
            retry: 0,
        }
    }
}

impl std::fmt::Display for Rcpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.address)
    }
}

impl PartialEq for Rcpt {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}
