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
}

impl Rcpt {
    /// create a new recipient from it's address.
    /// there is no transfer method by default.
    #[must_use]
    pub const fn new(address: Address) -> Self {
        Self {
            address,
            transfer_method: Transfer::None,
            email_status: EmailTransferStatus::Waiting,
        }
    }

    /// create a new recipient from it's address & transfer method.
    #[must_use]
    pub const fn with_transfer_method(address: Address, method: Transfer) -> Self {
        Self {
            address,
            transfer_method: method,
            email_status: EmailTransferStatus::Waiting,
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

/// filter recipients by their transfer method.
#[must_use]
pub fn filter_by_transfer_method(
    rcpt: &[Rcpt],
) -> std::collections::HashMap<crate::transfer::Transfer, Vec<crate::rcpt::Rcpt>> {
    rcpt.iter()
        .fold(std::collections::HashMap::new(), |mut acc, rcpt| {
            let rcpt = rcpt.clone();
            if let Some(group) = acc.get_mut(&rcpt.transfer_method) {
                group.push(rcpt);
            } else {
                acc.insert(rcpt.transfer_method.clone(), vec![rcpt]);
            }

            acc
        })
}

#[cfg(test)]
mod test {

    use crate::address::Address;
    use crate::transfer::Transfer;

    use super::{filter_by_transfer_method, Rcpt};

    #[test]
    fn test_filter_by_transfer_method() {
        let filtered = filter_by_transfer_method(
            &vec![
                Rcpt::with_transfer_method(
                    Address::try_from("green@foo.com".to_string()).unwrap(),
                    Transfer::Deliver,
                ),
                Address::try_from("john@doe.com".to_string())
                    .unwrap()
                    .into(),
                Address::try_from("green@foo.com".to_string())
                    .unwrap()
                    .into(),
                Rcpt::with_transfer_method(
                    Address::try_from("green@bar.com".to_string()).unwrap(),
                    Transfer::Deliver,
                ),
                Rcpt::with_transfer_method(
                    Address::try_from("john@localhost".to_string()).unwrap(),
                    Transfer::Mbox,
                ),
                Rcpt::with_transfer_method(
                    Address::try_from("green@localhost".to_string()).unwrap(),
                    Transfer::Mbox,
                ),
                Rcpt::with_transfer_method(
                    Address::try_from("satan@localhost".to_string()).unwrap(),
                    Transfer::Mbox,
                ),
                Rcpt::with_transfer_method(
                    Address::try_from("user@localhost".to_string()).unwrap(),
                    Transfer::Maildir,
                ),
            ][..],
        );

        assert!(filtered
            .get(&Transfer::None)
            .unwrap()
            .iter()
            .all(|rcpt| rcpt.transfer_method == Transfer::None));
        assert_eq!(filtered.get(&Transfer::None).unwrap().len(), 2);
        assert!(filtered
            .get(&Transfer::Deliver)
            .unwrap()
            .iter()
            .all(|rcpt| rcpt.transfer_method == Transfer::Deliver));
        assert_eq!(filtered.get(&Transfer::Deliver).unwrap().len(), 2);
        assert!(filtered
            .get(&Transfer::Mbox)
            .unwrap()
            .iter()
            .all(|rcpt| rcpt.transfer_method == Transfer::Mbox));
        assert_eq!(filtered.get(&Transfer::Mbox).unwrap().len(), 3);
        assert!(filtered
            .get(&Transfer::Maildir)
            .unwrap()
            .iter()
            .all(|rcpt| rcpt.transfer_method == Transfer::Maildir));
        assert_eq!(filtered.get(&Transfer::Maildir).unwrap().len(), 1);
    }
}
