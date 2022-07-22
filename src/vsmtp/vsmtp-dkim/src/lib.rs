//! vSMTP DKIM library
//!
//! The implementation follow the RFC 6376
//! ```txt
//! DomainKeys Identified Mail (DKIM) permits a person, role, or
//! organization that owns the signing domain to claim some
//! responsibility for a message by associating the domain with the
//! message.  This can be an author's organization, an operational relay,
//! or one of their agents.  DKIM separates the question of the identity
//! of the Signer of the message from the purported author of the
//! message.  Assertion of responsibility is validated through a
//! cryptographic signature and by querying the Signer's domain directly
//! to retrieve the appropriate public key.  Message transit from author
//! to recipient is through relays that typically make no substantive
//! change to the message content and thus preserve the DKIM signature.
//! ```

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

#![doc(html_no_source)]
#![deny(missing_docs)]
#![deny(unsafe_code)]
//
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
//
#![allow(clippy::use_self)] // false positive with enums

mod algorithm;
mod canonicalization;
mod public_key;
mod sign;
mod signature;
mod verify;

#[cfg(test)]
mod tests {
    mod verify;
}

pub use algorithm::{HashAlgorithm, SigningAlgorithm};
pub use canonicalization::{Canonicalization, CanonicalizationAlgorithm};
pub use public_key::PublicKey;
pub use signature::Signature;
pub use verify::VerifierError;
