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
    listeners: Vec<std::net::TcpListener>,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    config: std::sync::Arc<ServerConfig>,
    _phantom: std::marker::PhantomData<R>,
}

impl<R> ServerVSMTP<R>
where
    R: DataEndResolver + std::marker::Send,
{
    pub fn new(config: std::sync::Arc<ServerConfig>) -> Result<Self, Box<dyn std::error::Error>> {
        log4rs::init_config(Self::get_logger_config(&config)?)?;

        Ok(Self {
            listeners: config
                .server
                .addr
                .iter()
                .filter_map(|addr| std::net::TcpListener::bind(addr).ok())
                .map(|listener| {
                    listener.set_nonblocking(true).unwrap();
                    listener
                })
                .collect::<Vec<_>>(),
            tls_config: if config.tls.security_level == TlsSecurityLevel::None {
                None
            } else {
                Some(Self::get_rustls_config(&config))
            },
            config,
            _phantom: std::marker::PhantomData,
        })
    }

    fn get_logger_config(
        config: &ServerConfig,
    ) -> Result<log4rs::Config, log4rs::config::runtime::ConfigErrors> {
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
            .build(
                config.log.file.clone(), // .unwrap_or_else(|_| "vsmtp.log".to_string()),
            )
            .unwrap();

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
    }

    fn get_cert_from_file(cert_path: &str) -> Result<Vec<rustls::Certificate>, std::io::Error> {
        let cert_file = std::fs::File::open(&cert_path)?;
        let mut reader = std::io::BufReader::new(cert_file);
        rustls_pemfile::certs(&mut reader).map(|certs| {
            certs
                .into_iter()
                .map(rustls::Certificate)
                .collect::<Vec<_>>()
        })
    }

    fn get_signing_key_from_file(
        chain_path: &str,
    ) -> Result<std::sync::Arc<dyn rustls::sign::SigningKey>, std::io::Error> {
        // NOTE: ?
        // The chain files MUST start with the private key,
        // with the certificate chain next, starting with the leaf
        // (server) certificate, and then the issuer certificates.

        let key_file = std::fs::File::open(&chain_path)?;
        let mut reader = std::io::BufReader::new(key_file);

        let private_keys_rsa = rustls_pemfile::rsa_private_keys(&mut reader)?
            .into_iter()
            .map(rustls::PrivateKey)
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

        let mut tls_sni_resolver = rustls::server::ResolvesServerCertUsingSni::new();

        if let Some(x) = config.tls.sni_maps.as_ref() {
            x.iter()
                .filter_map(|sni| {
                    Some((
                        sni.domain.clone(),
                        rustls::sign::CertifiedKey {
                            cert: match Self::get_cert_from_file(
                                &sni.cert
                                    .replace("{capath}", capath)
                                    .replace("{domain}", &sni.domain),
                            ) {
                                Ok(cert) => cert,
                                Err(e) => {
                                    log::error!("error: {}", e);
                                    return None;
                                }
                            },
                            key: match Self::get_signing_key_from_file(
                                &sni.chain
                                    .replace("{capath}", capath)
                                    .replace("{domain}", &sni.domain),
                            ) {
                                Ok(key) => key,
                                Err(e) => {
                                    log::error!("error: {}", e);
                                    return None;
                                }
                            },
                            // TODO:
                            ocsp: None,
                            sct_list: None,
                        },
                    ))
                })
                .for_each(|(domain, ck)| tls_sni_resolver.add(&domain, ck).unwrap())
        }

        let mut out = rustls::ServerConfig::builder()
            .with_cipher_suites(rustls::ALL_CIPHER_SUITES)
            .with_kx_groups(&rustls::ALL_KX_GROUPS)
            .with_protocol_versions(rustls::ALL_VERSIONS)
            .expect("inconsistent cipher-suites/versions specified")
            .with_client_cert_verifier(rustls::server::NoClientAuth::new())
            .with_cert_resolver(std::sync::Arc::new(tls_sni_resolver));

        out.ignore_client_order = config.tls.preempt_cipherlist; //.unwrap_or(false);

        std::sync::Arc::new(out)
    }

    pub fn addr(&self) -> Vec<std::net::SocketAddr> {
        self.listeners
            .iter()
            .filter_map(|i| std::net::TcpListener::local_addr(i).ok())
            .collect::<Vec<_>>()
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
            for i in &self.listeners {
                match i.accept() {
                    Ok((stream, addr)) => {
                        let _ = self.handle_client(stream, addr);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                    Err(e) => {
                        log::error!("Error accepting socket; error = {:?}", e);
                        // TODO: pop listener ?
                    }
                }
            }
        }
    }
}
