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
#[derive(Debug, Clone, Eq, serde::Serialize, serde::Deserialize)]
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
            transfer_method: Transfer::Deliver,
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

impl PartialEq for Rcpt {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

/// filter recipients by their transfer method.
#[must_use]
pub fn filter_by_transfer_method(rcpt: &[Rcpt]) -> std::collections::HashMap<Transfer, Vec<Rcpt>> {
    let mut output: std::collections::HashMap<Transfer, Vec<Rcpt>> =
        std::collections::HashMap::new();
    for i in rcpt.iter().cloned() {
        if let Some(group) = output.get_mut(&i.transfer_method) {
            group.push(i);
        } else {
            output.insert(i.transfer_method.clone(), vec![i]);
        }
    }
    output
}

/// filter recipients by domain name using mutable reference on the recipients.
#[must_use]
pub fn filter_by_domain_mut(
    rcpt: &mut [Rcpt],
) -> std::collections::HashMap<String, Vec<&mut Rcpt>> {
    let mut acc = std::collections::HashMap::<String, Vec<&mut Rcpt>>::new();
    for rcpt in rcpt.iter_mut() {
        if let Some(domain) = acc.get_mut(rcpt.address.domain()) {
            domain.push(rcpt);
        } else {
            acc.insert(rcpt.address.domain().to_string(), vec![rcpt]);
        }
    }
    acc
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_test_rcpt() -> Vec<Rcpt> {
        vec![
            Rcpt::with_transfer_method(addr!("green@foo.com"), Transfer::None),
            addr!("john@doe.com").into(),
            addr!("green@foo.com").into(),
            Rcpt::with_transfer_method(addr!("green@bar.com"), Transfer::None),
            Rcpt::with_transfer_method(addr!("john@localhost"), Transfer::Mbox),
            Rcpt::with_transfer_method(addr!("green@localhost"), Transfer::Mbox),
            Rcpt::with_transfer_method(addr!("satan@localhost"), Transfer::Mbox),
            Rcpt::with_transfer_method(addr!("user@localhost"), Transfer::Maildir),
        ]
    }

    #[test]
    fn test_filter_by_transfer_method() {
        let filtered = filter_by_transfer_method(&get_test_rcpt()[..]);

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

    #[test]
    fn test_filter_by_domain_mut() {
        let mut rcpt = get_test_rcpt();
        let filtered = super::filter_by_domain_mut(&mut rcpt);

        assert!(filtered
            .get("foo.com")
            .unwrap()
            .iter()
            .all(|rcpt| rcpt.address.domain() == "foo.com"));
        assert_eq!(filtered.get("foo.com").unwrap().len(), 2);
        assert!(filtered
            .get("doe.com")
            .unwrap()
            .iter()
            .all(|rcpt| rcpt.address.domain() == "doe.com"));
        assert_eq!(filtered.get("doe.com").unwrap().len(), 1);
        assert!(filtered
            .get("bar.com")
            .unwrap()
            .iter()
            .all(|rcpt| rcpt.address.domain() == "bar.com"));
        assert_eq!(filtered.get("bar.com").unwrap().len(), 1);
        assert!(filtered
            .get("localhost")
            .unwrap()
            .iter()
            .all(|rcpt| rcpt.address.domain() == "localhost"));
        assert_eq!(filtered.get("localhost").unwrap().len(), 4);
    }
}
