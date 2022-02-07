/**
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
**/
use crate::config::server_config::InnerSmtpsConfig;

fn get_signing_key_from_file(
    rsa_path: &str,
) -> anyhow::Result<std::sync::Arc<dyn rustls::sign::SigningKey>> {
    let rsa_file = std::fs::File::open(&rsa_path)?;
    let mut reader = std::io::BufReader::new(rsa_file);

    let private_keys_rsa = rustls_pemfile::read_one(&mut reader)?
        .into_iter()
        .map(|i| match i {
            rustls_pemfile::Item::X509Certificate(i) => rustls::PrivateKey(i),
            rustls_pemfile::Item::RSAKey(i) => rustls::PrivateKey(i),
            rustls_pemfile::Item::PKCS8Key(i) => rustls::PrivateKey(i),
        })
        .collect::<Vec<_>>();

    if let Some(key) = private_keys_rsa.first() {
        rustls::sign::any_supported_type(key)
            .map_err(|_| anyhow::anyhow!("cannot parse signing key"))
    } else {
        anyhow::bail!("private key missing")
    }
}

pub(crate) fn get_cert_from_file(fullchain_path: &str) -> anyhow::Result<Vec<rustls::Certificate>> {
    let fullchain_file = std::fs::File::open(&fullchain_path)?;
    let mut reader = std::io::BufReader::new(fullchain_file);
    Ok(rustls_pemfile::certs(&mut reader).map(|certs| {
        certs
            .into_iter()
            .map(rustls::Certificate)
            .collect::<Vec<_>>()
    })?)
}

pub fn get_rustls_config(
    server_domain: &str,
    config: &InnerSmtpsConfig,
) -> anyhow::Result<rustls::ServerConfig> {
    let capath = config.capath.as_ref().unwrap();

    let mut sni_resolver = rustls::server::ResolvesServerCertUsingSni::new();

    if let Some(x) = config.sni_maps.as_ref() {
        x.iter()
            .map(|sni| {
                Ok((
                    sni.domain.clone(),
                    rustls::sign::CertifiedKey {
                        cert: get_cert_from_file(
                            &sni.fullchain
                                .replace("{capath}", capath)
                                .replace("{domain}", &sni.domain),
                        )?,
                        key: get_signing_key_from_file(
                            &sni.private_key
                                .replace("{capath}", capath)
                                .replace("{domain}", &sni.domain),
                        )?,
                        // TODO:
                        ocsp: None,
                        sct_list: None,
                    },
                ))
            })
            // .filter(Result::is_ok)
            .for_each(
                |sni: anyhow::Result<(String, rustls::sign::CertifiedKey)>| match sni {
                    Ok((domain, ck)) => sni_resolver
                        .add(&domain, ck)
                        .expect("Failed to add sni resolver"),
                    Err(e) => log::error!("{}", e),
                },
            )
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

    let mut out = rustls::ServerConfig::builder()
        .with_cipher_suites(rustls::ALL_CIPHER_SUITES)
        .with_kx_groups(&rustls::ALL_KX_GROUPS)
        // FIXME:
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .expect("inconsistent cipher-suites/versions specified")
        .with_client_cert_verifier(rustls::server::NoClientAuth::new())
        .with_cert_resolver(std::sync::Arc::new(CertResolver {
            sni_resolver,
            cert: Some(std::sync::Arc::new(rustls::sign::CertifiedKey {
                cert: get_cert_from_file(
                    &config
                        .fullchain
                        .replace("{capath}", capath)
                        .replace("{domain}", server_domain),
                )?,
                key: get_signing_key_from_file(
                    &config
                        .private_key
                        .replace("{capath}", capath)
                        .replace("{domain}", server_domain),
                )?,
                // TODO:
                ocsp: None,
                sct_list: None,
            })),
        }));

    out.ignore_client_order = config.preempt_cipherlist;

    struct TlsLogger;
    impl rustls::KeyLog for TlsLogger {
        fn log(&self, label: &str, client_random: &[u8], secret: &[u8]) {
            log::trace!("{} {:?} {:?}", label, client_random, secret);
        }
    }
    out.key_log = std::sync::Arc::new(TlsLogger {});

    Ok(out)
}
