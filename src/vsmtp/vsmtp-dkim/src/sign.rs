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
    signature::QueryMethod, Canonicalization, CanonicalizationAlgorithm, Signature,
    SigningAlgorithm,
};
use vsmtp_common::RawBody;

impl Signature {
    /// # Errors
    pub fn sign(
        message: &RawBody,
        selector: &str,
        sdid: &str,
        headers_field: Vec<String>,
        private_key: &rsa::RsaPrivateKey,
    ) -> Result<Self, rsa::errors::Error> {
        let mut signature = Signature {
            version: 1,
            signing_algorithm: SigningAlgorithm::RsaSha256,
            sdid: String::default(),
            selector: String::default(),
            canonicalization: Canonicalization {
                header: CanonicalizationAlgorithm::Relaxed,
                body: CanonicalizationAlgorithm::Relaxed,
            },
            query_method: vec![QueryMethod::default()],
            auid: String::default(),
            signature_timestamp: None,
            expire_time: None,
            body_length: None,
            headers_field: vec![],
            copy_header_fields: None,
            body_hash: String::default(),
            signature: String::default(),
            raw: String::default(),
        };

        let body_hash = signature.canonicalization.body.canonicalize_body(
            &message
                .body()
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
        );

        signature.sdid = sdid.to_string();
        signature.headers_field = headers_field;
        signature.selector = selector.to_string();
        signature.body_hash = base64::encode(signature.signing_algorithm.hash(&body_hash));

        signature.raw = signature.to_string();

        let headers_hash = signature.get_header_hash(message);
        let headers_hash = private_key.sign(
            rsa::PaddingScheme::PKCS1v15Sign {
                hash: Some(match signature.signing_algorithm {
                    SigningAlgorithm::RsaSha1 => rsa::hash::Hash::SHA1,
                    SigningAlgorithm::RsaSha256 => rsa::hash::Hash::SHA2_256,
                }),
            },
            &headers_hash,
        )?;
        signature.signature = base64::encode(headers_hash);

        signature.raw.push_str(&signature.signature);

        Ok(signature)
    }
}

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "DKIM-Signature: v={}; a={}; d={}; s={};\r\n\tc={}; q={}; h={};\r\n\tbh={};\r\n\tb={}",
            self.version,
            self.signing_algorithm,
            self.sdid,
            self.selector,
            self.canonicalization,
            self.query_method
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(":"),
            self.headers_field
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(":"),
            self.body_hash,
            self.signature
        ))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rsa::pkcs8::EncodePublicKey;
    use vsmtp_common::MessageBody;

    use crate::{
        public_key::{Type, Version},
        HashAlgorithm, PublicKey, Signature,
    };

    #[test]
    fn sign_and_verify() {
        let mut rng = rand::thread_rng();

        let private_key = rsa::RsaPrivateKey::new(&mut rng, 1024).unwrap();
        let public_key = rsa::RsaPublicKey::from(&private_key);

        let mut message = MessageBody::try_from(concat!(
            "From: toto@com\r\n",
            "To: tata@com\r\n",
            "Subject: test\r\n",
            "Date: Mon, 1 Jan 2020 00:00:00 +0000\r\n",
            "\r\n",
            "test\r\n",
        ))
        .unwrap();

        let signature = Signature::sign(
            message.inner(),
            "foobar",
            "localhost",
            vec![
                "From".to_string(),
                "To".to_string(),
                "Subject".to_string(),
                "Date".to_string(),
            ],
            &private_key,
        )
        .unwrap();

        message.add_header("DKIM-Signature", &signature.raw["DKIM-Signature: ".len()..]);

        let key = PublicKey {
            version: Version::Dkim1,
            acceptable_hash_algorithms: vec![HashAlgorithm::Sha256],
            r#type: Type::Rsa,
            notes: None,
            public_key: public_key.to_public_key_der().unwrap().as_ref().to_vec(),
            service_type: vec![],
            flags: vec![],
        };

        signature.verify(message.inner(), &key).unwrap();
    }
}
