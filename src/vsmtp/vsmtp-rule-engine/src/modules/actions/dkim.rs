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

use crate::modules::types::types::{Message, Server};
use crate::modules::EngineResult;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use rhai::EvalAltResult;
use vsmtp_common::dkim::{Key, Signature};
use vsmtp_common::mail_context::MessageBody;
use vsmtp_common::re::{base64, log, tokio};
use vsmtp_common::state::StateSMTP;

#[doc(hidden)]
#[rhai::plugin::export_module]
pub mod dkim {
    use vsmtp_common::dkim::SigningAlgorithm;

    #[rhai_fn(global, return_raw)]
    pub fn parse_signature(input: &str) -> EngineResult<Signature> {
        <Signature as std::str::FromStr>::from_str(input)
            .map_err::<Box<rhai::EvalAltResult>, _>(|e| format!("{e}").into())
    }

    #[rhai_fn(global, pure, return_raw)]
    pub fn get_public_key(
        server: &mut Server,
        signature: Signature,
        on_multiple_key_records: &str,
    ) -> EngineResult<rhai::Dynamic> {
        const VALID_POLICY: [&str; 2] = ["first", "cycle"];
        if !VALID_POLICY.contains(&on_multiple_key_records) {
            return Err(format!(
                "expected values in `{}` but got `{on_multiple_key_records}`",
                VALID_POLICY.join(",")
            )
            .into());
        }

        let resolver = server.resolvers.get(&server.config.server.domain).unwrap();

        let result = tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current()
                .block_on(async move { resolver.txt_lookup(signature.get_dns_query()).await })
        })
        .map_err::<Box<EvalAltResult>, _>(|e| format!("{e}").into())?;

        let keys = result.into_iter().filter_map(|i| {
            match <Key as std::str::FromStr>::from_str(&format!("{i}")) {
                Ok(key) => Some(key),
                Err(e) => {
                    println!("got error with key: `{e}`");
                    None
                }
            }
        });

        Ok(if on_multiple_key_records == "first" {
            keys.take(1).collect::<Vec<_>>()
        } else {
            keys.collect::<Vec<_>>()
        }
        .into())
    }

    #[allow(clippy::module_name_repetitions, clippy::needless_pass_by_value)]
    #[rhai_fn(global, pure, return_raw)]
    pub fn dkim_verify(message: &mut Message, signature: Signature, key: Key) -> EngineResult<()> {
        let guard = vsl_guard_ok!(message.read());
        let message = vsl_missing_ok!(guard, "message", StateSMTP::PreQ);

        if !signature
            .signing_algorithm
            .is_supported(&key.acceptable_hash_algorithms)
        {
            return Err(format!(
                "the `signing_algorithm` ({}) is not suitable for the `acceptable_hash_algorithms` ({})",
                signature.signing_algorithm,
                key.acceptable_hash_algorithms.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")
            )
            .into());
        }

        if key.public_key.is_empty() {
            return Err("the key has been revoked".into());
        }

        println!("signature = {signature:?}");
        println!("key = {key:?}");

        let body = signature
            .canonicalization
            .body
            .canonicalize(&match &message {
                MessageBody::Raw { body, .. } => body.clone(),
                MessageBody::Parsed(mail) => format!("{}", mail.body),
            });

        let body_hash = signature
            .signing_algorithm
            .hash(match signature.body_length {
                // TODO: handle policy
                Some(len) => &body[..std::cmp::min(body.len(), len)],
                None => &body,
            });
        if signature.body_hash != body_hash {
            return Err("body hash does not match".into());
        }

        let headers = signature
            .canonicalization
            .header
            // TODO: filter
            .canonicalize(&match &message {
                MessageBody::Raw { headers, .. } => headers.join("\r\n"),
                MessageBody::Parsed(mail) => format!("{}", mail.headers),
            });

        let key = <rsa::RsaPublicKey as rsa::pkcs8::DecodePublicKey>::from_public_key_der(
            &key.public_key,
        )
        .map_err::<Box<EvalAltResult>, _>(|e| format!("{e}").into())?;

        if let Err(e) = rsa::PublicKey::verify(
            &key,
            rsa::PaddingScheme::PKCS1v15Sign {
                hash: Some(match signature.signing_algorithm {
                    SigningAlgorithm::RsaSha1 => rsa::hash::Hash::SHA1,
                    SigningAlgorithm::RsaSha256 => rsa::hash::Hash::SHA2_256,
                }),
            },
            headers.as_bytes(),
            &signature.signature,
        ) {
            println!("{e}");
        }

        Ok(())
    }
}
