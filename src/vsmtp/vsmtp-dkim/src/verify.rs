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

use super::{Key, Signature, SigningAlgorithm};
use vsmtp_common::MessageBody;

#[derive(Debug, thiserror::Error)]
pub enum VerifierResult {
    #[error(
        "the `signing_algorithm` ({singing_algorithm}) is not suitable for the `acceptable_hash_algorithms` ({acceptable})"
    )]
    AlgorithmMismatch {
        singing_algorithm: SigningAlgorithm,
        acceptable: String,
    },
    #[error("the key has been revoked, or is empty")]
    KeyMissingOrRevoked,
    #[error("body hash does not match")]
    BodyHashMismatch,
    #[error("headers hash does not match, got `{error}`")]
    HeaderHashMismatch { error: rsa::errors::Error },
    #[error("base64 error")]
    Base64Error,
}

///
/// # Errors
///
/// * see [`VerifierResult`]
pub fn verify(
    message: &MessageBody,
    signature: &Signature,
    key: &Key,
) -> Result<(), VerifierResult> {
    if !signature
        .signing_algorithm
        .is_supported(&key.acceptable_hash_algorithms)
    {
        return Err(VerifierResult::AlgorithmMismatch {
            singing_algorithm: signature.signing_algorithm,
            acceptable: key
                .acceptable_hash_algorithms
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        });
    }

    // if key.public_key.is_empty() {
    //     return Err(VerifierResult::KeyMissingOrRevoked);
    // }

    let body = signature
        .canonicalization
        .body
        .canonicalize_body(message.to_string());

    let body_hash = signature
        .signing_algorithm
        .hash(match signature.body_length {
            // TODO: handle policy
            Some(len) => &body[..std::cmp::min(body.len(), len)],
            None => &body,
        });
    if signature.body_hash != base64::encode(body_hash) {
        return Err(VerifierResult::BodyHashMismatch);
    }

    let headers_hash = signature.get_header_hash(message);

    rsa::PublicKey::verify(
        &key.public_key,
        rsa::PaddingScheme::PKCS1v15Sign {
            hash: Some(match signature.signing_algorithm {
                SigningAlgorithm::RsaSha1 => rsa::hash::Hash::SHA1,
                SigningAlgorithm::RsaSha256 => rsa::hash::Hash::SHA2_256,
            }),
        },
        &headers_hash,
        &base64::decode(&signature.signature).map_err(|_| VerifierResult::Base64Error)?,
    )
    .map_err(|e| VerifierResult::HeaderHashMismatch { error: e })?;

    Ok(())
}
