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
use rustls::ALL_CIPHER_SUITES;
use vsmtp_common::re::{anyhow, log};

use crate::field::{FieldServerTls, FieldServerVirtual};

struct TlsLogger;
impl rustls::KeyLog for TlsLogger {
    fn log(&self, label: &str, client_random: &[u8], secret: &[u8]) {
        log::trace!("{} {:?} {:?}", label, client_random, secret);
    }
}

struct CertResolver {
    sni_resolver: rustls::server::ResolvesServerCertUsingSni,
    cert: Option<std::sync::Arc<rustls::sign::CertifiedKey>>,
}

impl rustls::server::ResolvesServerCert for CertResolver {
    fn resolve(
        &self,
        client_hello: rustls::server::ClientHello,
    ) -> Option<std::sync::Arc<rustls::sign::CertifiedKey>> {
        self.sni_resolver
            .resolve(client_hello)
            .or_else(|| self.cert.clone())
    }
}

static JUST_TLS1_2: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS12];

static JUST_TLS1_3: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];

static ALL_VERSIONS: &[&rustls::SupportedProtocolVersion] =
    &[&rustls::version::TLS13, &rustls::version::TLS12];

fn to_supported_cipher_suite(
    cipher_suite: &[rustls::CipherSuite],
) -> Vec<rustls::SupportedCipherSuite> {
    ALL_CIPHER_SUITES
        .iter()
        .filter(|i| cipher_suite.iter().any(|x| *x == i.suite()))
        .copied()
        .collect::<Vec<_>>()
}

#[doc(hidden)]
pub fn get_rustls_config(
    config: &FieldServerTls,
    virtual_entries: &std::collections::BTreeMap<String, FieldServerVirtual>,
) -> anyhow::Result<rustls::ServerConfig> {
    let protocol_version = match (
        config
            .protocol_version
            .iter()
            .any(|i| *i == rustls::ProtocolVersion::TLSv1_2),
        config
            .protocol_version
            .iter()
            .any(|i| *i == rustls::ProtocolVersion::TLSv1_3),
    ) {
        (true, true) => ALL_VERSIONS,
        (true, false) => JUST_TLS1_2,
        (false, true) => JUST_TLS1_3,
        (false, false) => anyhow::bail!("requested version is not supported"),
    };

    let mut out = rustls::ServerConfig::builder()
        .with_cipher_suites(&to_supported_cipher_suite(&config.cipher_suite))
        .with_kx_groups(&rustls::ALL_KX_GROUPS)
        .with_protocol_versions(protocol_version)
        .map_err(|e| anyhow::anyhow!("cannot initialize tls config: '{e}'"))?
        .with_client_cert_verifier(rustls::server::NoClientAuth::new())
        .with_cert_resolver(std::sync::Arc::new(CertResolver {
            sni_resolver: virtual_entries.iter().fold(
                anyhow::Ok(rustls::server::ResolvesServerCertUsingSni::new()),
                |sni_resolver, (domain, entry)| {
                    let mut sni_resolver = sni_resolver?;

                    // using root certificate and private key if tls parameters are not defined in
                    // the virtual domain.
                    let (certificate, private_key) = {
                        entry.tls.as_ref().map_or_else(
                            || (&config.certificate.inner, &config.private_key.inner),
                            |tls| (&tls.certificate.inner, &tls.private_key.inner),
                        )
                    };

                    sni_resolver
                        .add(
                            domain,
                            rustls::sign::CertifiedKey {
                                cert: vec![certificate.clone()],
                                key: rustls::sign::any_supported_type(private_key)?,
                                ocsp: None,
                                sct_list: None,
                            },
                        )
                        .map_err(|e| anyhow::anyhow!("cannot add sni to resolver: {e}"))?;

                    Ok(sni_resolver)
                },
            )?,
            cert: Some(std::sync::Arc::new(rustls::sign::CertifiedKey {
                cert: vec![config.certificate.inner.clone()],
                key: rustls::sign::any_supported_type(&config.private_key.inner)?,
                ocsp: None,
                sct_list: None,
            })),
        }));

    out.ignore_client_order = config.preempt_cipherlist;

    out.key_log = std::sync::Arc::new(TlsLogger {});

    Ok(out)
}
