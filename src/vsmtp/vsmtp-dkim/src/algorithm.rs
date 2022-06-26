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

///
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, strum::EnumString, strum::Display)]
pub enum SigningAlgorithm {
    ///
    #[strum(serialize = "rsa-sha1")]
    RsaSha1,
    ///
    #[strum(serialize = "rsa-sha256")]
    RsaSha256,
}

impl SigningAlgorithm {
    ///
    #[must_use]
    pub fn is_supported(&self, hash_algo: &[HashAlgorithm]) -> bool {
        hash_algo.iter().any(|a| match (a, self) {
            (HashAlgorithm::Sha1, SigningAlgorithm::RsaSha1)
            | (HashAlgorithm::Sha256, SigningAlgorithm::RsaSha256) => true,
            (HashAlgorithm::Sha1, SigningAlgorithm::RsaSha256)
            | (HashAlgorithm::Sha256, SigningAlgorithm::RsaSha1) => false,
        })
    }

    ///
    #[must_use]
    pub fn hash<T: AsRef<[u8]>>(self, data: T) -> Vec<u8> {
        match self {
            SigningAlgorithm::RsaSha1 => {
                let mut digest = <sha1::Sha1 as sha1::Digest>::new();
                sha1::Digest::update(&mut digest, data);
                sha1::Digest::finalize(digest).to_vec()
            }
            SigningAlgorithm::RsaSha256 => {
                let mut digest = <sha2::Sha256 as sha2::Digest>::new();
                sha2::Digest::update(&mut digest, data);
                sha2::Digest::finalize(digest).to_vec()
            }
        }
    }
}

///
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, strum::EnumIter, strum::EnumString, strum::Display)]
#[strum(serialize_all = "lowercase")]
pub enum HashAlgorithm {
    ///
    Sha1,
    ///
    Sha256,
}
