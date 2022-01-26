use crate::config::{default::DEFAULT_CONFIG, server_config::ServerConfig};

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

fn get_cert_from_file(fullchain_path: &str) -> std::io::Result<Vec<rustls::Certificate>> {
    let fullchain_file = std::fs::File::open(&fullchain_path)?;
    let mut reader = std::io::BufReader::new(fullchain_file);
    rustls_pemfile::certs(&mut reader).map(|certs| {
        certs
            .into_iter()
            .map(rustls::Certificate)
            .collect::<Vec<_>>()
    })
}

pub fn get_rustls_config(config: &ServerConfig) -> std::sync::Arc<rustls::ServerConfig> {
    let capath_if_missing_from_both = String::default();
    let capath = config
        .tls
        .capath
        .as_ref()
        .or_else(|| DEFAULT_CONFIG.tls.capath.as_ref())
        .unwrap_or(&capath_if_missing_from_both);

    let mut sni_resolver = rustls::server::ResolvesServerCertUsingSni::new();

    if let Some(x) = config.tls.sni_maps.as_ref() {
        x.iter()
            .filter_map(|sni| {
                Some((
                    sni.domain.clone(),
                    rustls::sign::CertifiedKey {
                        cert: match get_cert_from_file(
                            &sni.fullchain
                                .replace("{capath}", capath)
                                .replace("{domain}", &sni.domain),
                        ) {
                            Ok(cert) => cert,
                            Err(e) => {
                                log::error!("failed to get certificates: {}", e);
                                return None;
                            }
                        },
                        key: match get_signing_key_from_file(
                            &sni.private_key
                                .replace("{capath}", capath)
                                .replace("{domain}", &sni.domain),
                        ) {
                            Ok(key) => key,
                            Err(e) => {
                                log::error!("failed to get signing key: {}", e);
                                return None;
                            }
                        },
                        // TODO:
                        ocsp: None,
                        sct_list: None,
                    },
                ))
            })
            .for_each(|(domain, ck)| {
                sni_resolver
                    .add(&domain, ck)
                    .expect("Failed to add sni resolver")
            })
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
        // NOTE: cannot change version, we have no way to change it...
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .expect("inconsistent cipher-suites/versions specified")
        .with_client_cert_verifier(rustls::server::NoClientAuth::new())
        .with_cert_resolver(std::sync::Arc::new(CertResolver {
            sni_resolver,
            cert: config
                .tls
                .fullchain
                .as_ref()
                .or_else(|| DEFAULT_CONFIG.tls.fullchain.as_ref())
                .and_then(|fullchain| {
                    config
                        .tls
                        .private_key
                        .as_ref()
                        .or_else(|| DEFAULT_CONFIG.tls.private_key.as_ref())
                        .map(|private_key| (fullchain, private_key))
                })
                .and_then(|(fullchain, private_key)| {
                    Some((
                        match get_cert_from_file(
                            &fullchain
                                .replace("{capath}", capath)
                                .replace("{domain}", &config.domain),
                        ) {
                            Ok(cert) => cert,
                            Err(e) => {
                                log::error!("failed to get certificates: {}", e);
                                return None;
                            }
                        },
                        match get_signing_key_from_file(
                            &private_key
                                .replace("{capath}", capath)
                                .replace("{domain}", &config.domain),
                        ) {
                            Ok(key) => key,
                            Err(e) => {
                                log::error!("failed to get signing key: {}", e);
                                return None;
                            }
                        },
                    ))
                })
                .map(|(cert, key)| {
                    std::sync::Arc::new(rustls::sign::CertifiedKey {
                        cert,
                        key,
                        // TODO:
                        ocsp: None,
                        sct_list: None,
                    })
                }),
        }));

    out.ignore_client_order = config.tls.preempt_cipherlist;

    struct TlsLogger;
    impl rustls::KeyLog for TlsLogger {
        fn log(&self, label: &str, client_random: &[u8], secret: &[u8]) {
            log::trace!("{} {:?} {:?}", label, client_random, secret);
        }
    }
    out.key_log = std::sync::Arc::new(TlsLogger {});

    std::sync::Arc::new(out)
}
