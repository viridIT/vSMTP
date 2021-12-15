/**
 * vSMTP mail transfer agent
 * Copyright (C) 2021 viridIT SAS
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
use crate::config::default::DEFAULT_CONFIG;
use crate::config::server_config::{ServerConfig, TlsSecurityLevel};
use crate::mailprocessing::mail_receiver::MailReceiver;
use crate::resolver::DataEndResolver;

pub struct ServerVSMTP<R>
where
    R: DataEndResolver,
{
    listener: tokio::net::TcpListener,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    config: std::sync::Arc<ServerConfig>,
    _phantom: std::marker::PhantomData<R>,
}

impl<R> ServerVSMTP<R>
where
    R: DataEndResolver + std::marker::Send,
{
    pub async fn new(
        config: std::sync::Arc<ServerConfig>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        log4rs::init_config(Self::get_logger_config(&config)?)?;

        Ok(Self {
            listener: tokio::net::TcpListener::bind(&config.server.addr).await?,
            tls_config: if config.tls.security_level == TlsSecurityLevel::None {
                None
            } else {
                Some(Self::get_rustls_config(&config))
            },
            config,
            _phantom: std::marker::PhantomData,
        })
    }

    fn get_logger_config(config: &ServerConfig) -> Result<log4rs::Config, std::io::Error> {
        use log4rs::*;

        let console = append::console::ConsoleAppender::builder()
            .encoder(Box::new(encode::pattern::PatternEncoder::new(
                "{d(%Y-%m-%d %H:%M:%S)} {h({l:<5} {I})} ((line:{L:<3})) $ {m}{n}",
            )))
            .build();

        let file = append::file::FileAppender::builder()
            .encoder(Box::new(encode::pattern::PatternEncoder::new(
                "{d} - {m}{n}",
            )))
            .build(config.log.file.clone())?;

        Config::builder()
            .appender(config::Appender::builder().build("stdout", Box::new(console)))
            .appender(config::Appender::builder().build("file", Box::new(file)))
            .loggers(
                config
                    .log
                    .level
                    .iter()
                    .map(|(name, level)| config::Logger::builder().build(name, *level)),
            )
            .build(
                config::Root::builder()
                    .appender("stdout")
                    .appender("file")
                    .build(
                        *config
                            .log
                            .level
                            .get("default")
                            .unwrap_or(&log::LevelFilter::Warn),
                    ),
            )
            .map_err(|e| {
                e.errors().iter().for_each(|e| log::error!("{}", e));
                std::io::Error::new(std::io::ErrorKind::Other, e)
            })
    }

    fn get_cert_from_file(
        fullchain_path: &str,
    ) -> Result<Vec<rustls::Certificate>, std::io::Error> {
        let fullchain_file = std::fs::File::open(&fullchain_path)?;
        let mut reader = std::io::BufReader::new(fullchain_file);
        rustls_pemfile::certs(&mut reader).map(|certs| {
            certs
                .into_iter()
                .map(rustls::Certificate)
                .collect::<Vec<_>>()
        })
    }

    fn get_signing_key_from_file(
        rsa_path: &str,
    ) -> Result<std::sync::Arc<dyn rustls::sign::SigningKey>, std::io::Error> {
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
            rustls::sign::any_supported_type(key).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::Other, "cannot parse signing key")
            })
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "private key missing",
            ))
        }
    }

    fn get_rustls_config(config: &ServerConfig) -> std::sync::Arc<rustls::ServerConfig> {
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
                            cert: match Self::get_cert_from_file(
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
                            key: match Self::get_signing_key_from_file(
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

        struct TlsLogger;
        impl rustls::KeyLog for TlsLogger {
            fn log(&self, label: &str, client_random: &[u8], secret: &[u8]) {
                log::trace!("{} {:?} {:?}", label, client_random, secret);
            }
        }

        let mut out = rustls::ServerConfig::builder()
            .with_cipher_suites(rustls::ALL_CIPHER_SUITES)
            .with_kx_groups(&rustls::ALL_KX_GROUPS)
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
                            match Self::get_cert_from_file(
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
                            match Self::get_signing_key_from_file(
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
        out.key_log = std::sync::Arc::new(TlsLogger {});

        std::sync::Arc::new(out)
    }

    pub fn addr(&self) -> std::net::SocketAddr {
        self.listener
            .local_addr()
            .expect("cannot retrieve local address")
    }

    fn handle_client(
        &self,
        stream: std::net::TcpStream,
        client_addr: std::net::SocketAddr,
    ) -> Result<(), std::io::Error> {
        log::warn!("Connection from: {}", client_addr);
        let tls_config = self.tls_config.as_ref().map(std::sync::Arc::clone);
        let config = self.config.clone();

        stream.set_nonblocking(true)?;

        tokio::spawn(async move {
            let begin = std::time::SystemTime::now();
            log::warn!("Handling client: {}", client_addr);

            match MailReceiver::<R>::new(client_addr, tls_config, config)
                .receive_plain(stream)
                .await
            {
                Ok(_) => log::warn!(
                    "{{ elapsed: {:?} }} Connection {} closed cleanly",
                    begin.elapsed(),
                    client_addr,
                ),
                Err(e) => {
                    log::error!(
                        "{{ elapsed: {:?} }} Connection {} closed with an error {}",
                        begin.elapsed(),
                        client_addr,
                        e,
                    )
                }
            }

            std::io::Result::Ok(())
        });

        Ok(())
    }

    pub async fn listen_and_serve(&self) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            match self.listener.accept().await {
                Ok((stream, client_addr)) => self.handle_client(stream.into_std()?, client_addr)?,
                Err(e) => log::error!("Error accepting socket; error = {:?}", e),
            }
        }
    }
}
