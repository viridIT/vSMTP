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

use crate::modules::types::types::{Context, Message, Server};
use crate::modules::EngineResult;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use rhai::EvalAltResult;
use vsmtp_common::re::tokio;
use vsmtp_dkim::{PublicKey, Signature};

#[derive(Debug, strum::AsRefStr, strum::EnumString, strum::EnumMessage)]
#[strum(serialize_all = "snake_case")]
enum DkimErrors {
    #[strum(message = "neutral")]
    SignatureParsingFailed, // { inner: <Signature as std::str::FromStr>::Err, }
    #[strum(message = "neutral")]
    KeyParsingFailed, // { inner: <Key as std::str::FromStr>::Err, }
    #[strum(message = "neutral")]
    PolicySyntaxError, // { inner: String, }
    #[strum(message = "temperror")]
    TempDnsError, // { inner: trust_dns_resolver::error::ResolveError, }
    #[strum(message = "permerror")]
    PermDnsError, // { inner: trust_dns_resolver::error::ResolveError, }
    #[strum(message = "fail")]
    SignatureMismatch,
}

/// dkim api for verifier, and the generation of "Authentication-Results" header
#[rhai::plugin::export_module]
pub mod dkim {

    /// get the dkim status from an error produced by this module
    #[rhai_fn(global, return_raw)]
    pub fn handle_dkim_error(err: &str) -> EngineResult<String> {
        let r#type = DkimErrors::try_from(err).map_err::<Box<rhai::EvalAltResult>, _>(|e| {
            format!("not the right type: `{e}`").into()
        })?;

        Ok(strum::EnumMessage::get_message(&r#type)
            .expect("`DkimErrors` must have a message for each variant")
            .to_string())
    }

    /// return the `sdid` property of the [`Signature`]
    #[rhai_fn(global, get = "sdid", pure)]
    pub fn sdid(signature: &mut Signature) -> String {
        signature.sdid.clone()
    }

    /// return the `auid` property of the [`Signature`]
    #[rhai_fn(global, get = "auid", pure)]
    pub fn auid(signature: &mut Signature) -> String {
        signature.auid.clone()
    }

    /// create a [`Signature`] from a `DKIM-Signature` header
    #[rhai_fn(global, return_raw)]
    pub fn parse_signature(input: &str) -> EngineResult<Signature> {
        <Signature as std::str::FromStr>::from_str(input)
            .map_err::<Box<rhai::EvalAltResult>, _>(|_| DkimErrors::SignatureParsingFailed.into())
    }

    /// Has the signature expired?
    ///
    /// return `true` if the argument are invalid
    #[rhai_fn(global, pure)]
    pub fn has_expired(signature: &mut Signature, epsilon: rhai::INT) -> bool {
        epsilon
            .try_into()
            .map_or(true, |epsilon| signature.has_expired(epsilon))
    }

    /// Get the list of public keys associated with this [`Signature`]
    ///
    /// The current implementation will make a TXT query on the dns of the signer
    ///
    /// `on_multiple_key_records` value can be `first` or `cycle` :
    /// * `first` return the first key found (one element array)
    /// * `cycle` return all the keys found
    #[rhai_fn(global, pure, return_raw)]
    pub fn get_public_key(
        server: &mut Server,
        signature: Signature,
        on_multiple_key_records: &str,
    ) -> EngineResult<rhai::Dynamic> {
        const VALID_POLICY: [&str; 2] = ["first", "cycle"];
        if !VALID_POLICY.contains(&on_multiple_key_records) {
            return Err(DkimErrors::PolicySyntaxError.into());
            /*{
                inner: format!(
                    "expected values in `[first, cycle]` but got `{on_multiple_key_records}`",
                ),
            }*/
        }

        let resolver = server.resolvers.get(&server.config.server.domain).unwrap();

        let txt_record = tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current()
                .block_on(resolver.txt_lookup(signature.get_dns_query()))
        })
        .map_err::<Box<EvalAltResult>, _>(|e| {
            use trust_dns_resolver::error::ResolveErrorKind;
            if matches!(
                e.kind(),
                ResolveErrorKind::Message(_)
                    | ResolveErrorKind::Msg(_)
                    | ResolveErrorKind::NoConnections
                    | ResolveErrorKind::NoRecordsFound { .. }
            ) {
                DkimErrors::PermDnsError.into()
            } else {
                DkimErrors::TempDnsError.into()
            }
        })?;

        let keys = txt_record
            .into_iter()
            .map(|i| <PublicKey as std::str::FromStr>::from_str(&i.to_string()));

        Ok(if on_multiple_key_records == "first" {
            keys.take(1)
                .collect::<Result<Vec<_>, <PublicKey as std::str::FromStr>::Err>>()
                .map_err::<Box<EvalAltResult>, _>(|_| DkimErrors::KeyParsingFailed.into())?
        } else {
            keys.collect::<Result<Vec<_>, <PublicKey as std::str::FromStr>::Err>>()
                .map_err::<Box<EvalAltResult>, _>(|_| DkimErrors::KeyParsingFailed.into())?
        }
        .into())
    }

    /// A public key may contains a `debug flag`, used for testing purpose.
    #[rhai_fn(global, pure, get = "has_debug_flag")]
    pub fn has_debug_flag(key: &mut PublicKey) -> bool {
        key.has_debug_flag()
    }

    /// Operate the hashing of the `message`'s headers and body, and compare the result with the
    /// `signature` and `key` data.
    #[allow(clippy::module_name_repetitions, clippy::needless_pass_by_value)]
    #[rhai_fn(global, pure, return_raw)]
    pub fn dkim_verify(
        message: &mut Message,
        signature: Signature,
        key: PublicKey,
    ) -> EngineResult<()> {
        let guard = vsl_guard_ok!(message.read());

        signature
            .verify(guard.inner(), &key)
            .map_err::<Box<EvalAltResult>, _>(|_| DkimErrors::SignatureMismatch.into())
    }

    ///
    #[rhai_fn(global, pure, return_raw)]
    #[allow(clippy::module_name_repetitions, clippy::needless_pass_by_value)]
    pub fn dkim_sign(
        message: &mut Message,
        context: Context,
        server: Server,
        selector: &str,
        headers_field: rhai::Array,
    ) -> EngineResult<()> {
        let mut msg_guard = vsl_guard_ok!(message.write());
        let ctx_guard = vsl_guard_ok!(context.read());

        let sdid = &ctx_guard.connection.server_name;
        let dkim_params = server
            .config
            .server
            .r#virtual
            .get(sdid)
            .map_or_else(|| &server.config.server.dkim, |i| &i.dkim);

        match dkim_params {
            None => Err(format!("dkim params are empty for this `{sdid}`").into()),
            Some(dkim_params) => {
                let signature = Signature::sign(
                    msg_guard.inner(),
                    selector,
                    sdid,
                    headers_field.iter().map(ToString::to_string).collect(),
                    &dkim_params.private_key.inner,
                )
                .map_err::<Box<EvalAltResult>, _>(|e| e.to_string().into())?;

                msg_guard.add_header("DKIM-Signature", &signature.get_signature_value());

                Ok(())
            }
        }
    }
}
