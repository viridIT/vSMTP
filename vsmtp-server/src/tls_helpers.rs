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
use vsmtp_config::InnerSmtpsConfig;

fn get_signing_key_from_file(
    rsa_path: &std::path::Path,
) -> anyhow::Result<std::sync::Arc<dyn rustls::sign::SigningKey>> {
    let mut reader = std::io::BufReader::new(
        std::fs::File::open(&rsa_path)
            .map_err(|e| anyhow::anyhow!("{e}: '{}'", rsa_path.display()))?,
    );

    let private_keys_rsa = rustls_pemfile::read_one(&mut reader)?
        .into_iter()
        .map(|i| match i {
            rustls_pemfile::Item::RSAKey(i)
            | rustls_pemfile::Item::X509Certificate(i)
            | rustls_pemfile::Item::PKCS8Key(i)
            | rustls_pemfile::Item::ECKey(i) => rustls::PrivateKey(i),
            _ => todo!(),
        })
        .collect::<Vec<_>>();

    if let Some(key) = private_keys_rsa.first() {
        rustls::sign::any_supported_type(key)
            .map_err(|_| anyhow::anyhow!("cannot parse signing key: '{}'", rsa_path.display()))
    } else {
        anyhow::bail!("private key missing in file: '{}'", rsa_path.display())
    }
}

pub fn get_cert_from_file(
    fullchain_path: &std::path::Path,
) -> anyhow::Result<Vec<rustls::Certificate>> {
    let mut reader = std::io::BufReader::new(
        std::fs::File::open(&fullchain_path)
            .map_err(|e| anyhow::anyhow!("{e}: '{}'", fullchain_path.display()))?,
    );

    match rustls_pemfile::certs(&mut reader).map(|certs| {
        certs
            .into_iter()
            .map(rustls::Certificate)
            .collect::<Vec<_>>()
    })? {
        empty if empty.is_empty() => Err(anyhow::anyhow!(
            "Certificate file is empty: '{}'",
            fullchain_path.display()
        )),
        otherwise => Ok(otherwise),
    }
}

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

pub fn get_rustls_config(config: &InnerSmtpsConfig) -> anyhow::Result<rustls::ServerConfig> {
    let mut out = rustls::ServerConfig::builder()
        .with_cipher_suites(rustls::ALL_CIPHER_SUITES)
        .with_kx_groups(&rustls::ALL_KX_GROUPS)
        // FIXME:
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .map_err(|e| anyhow::anyhow!("cannot initialize tls config: '{e}'"))?
        .with_client_cert_verifier(rustls::server::NoClientAuth::new())
        .with_cert_resolver(std::sync::Arc::new(CertResolver {
            sni_resolver: config.sni_maps.as_ref().map_or_else(
                || anyhow::Ok(rustls::server::ResolvesServerCertUsingSni::new()),
                |x| {
                    x.iter().fold(
                        Ok(rustls::server::ResolvesServerCertUsingSni::new()),
                        |sni_resolver, sni| {
                            let mut sni_resolver = sni_resolver?;
                            sni_resolver
                                .add(
                                    &sni.domain,
                                    rustls::sign::CertifiedKey {
                                        cert: get_cert_from_file(&sni.fullchain)?,
                                        key: get_signing_key_from_file(&sni.private_key)?,
                                        ocsp: None,
                                        sct_list: None,
                                    },
                                )
                                .map_err(|e| anyhow::anyhow!("cannot add sni to resolver: {e}"))?;
                            Ok(sni_resolver)
                        },
                    )
                },
            )?,
            cert: Some(std::sync::Arc::new(rustls::sign::CertifiedKey {
                cert: get_cert_from_file(&config.fullchain)?,
                key: get_signing_key_from_file(&config.private_key)?,
                ocsp: None,
                sct_list: None,
            })),
        }));

    out.ignore_client_order = config.preempt_cipherlist;

    out.key_log = std::sync::Arc::new(TlsLogger {});

    Ok(out)
}
