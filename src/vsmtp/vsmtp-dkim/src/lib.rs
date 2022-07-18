//! vSMTP DKIM implementation
//!
//! Verifier and signer algorithm

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
mod key;
mod signature;
mod verify;

#[cfg(test)]
mod tests {
    mod verify;
}

pub use algorithm::{HashAlgorithm, SigningAlgorithm};
pub use canonicalization::{Canonicalization, CanonicalizationAlgorithm};
pub use key::Key;
pub use signature::Signature;
