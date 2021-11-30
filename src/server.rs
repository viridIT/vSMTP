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
use crate::mailprocessing::mail_receiver::MailReceiver;
use crate::resolver::DataEndResolver;

pub struct ServerVSMTP<R>
where
    R: DataEndResolver,
{
    listeners: Vec<std::net::TcpListener>,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    tls_security_level: TlsSecurityLevel,
    _phantom: std::marker::PhantomData<R>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TlsSecurityLevel {
    None,
    May,
    Encrypt,
}

impl std::str::FromStr for TlsSecurityLevel {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(TlsSecurityLevel::None),
            "may" => Ok(TlsSecurityLevel::May),
            "encrypt" => Ok(TlsSecurityLevel::Encrypt),
            _ => Err("not a valid value"),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct SniKey {
    domain: String,
    cert: String,
    chain: String,
}

impl<R> ServerVSMTP<R>
where
    R: DataEndResolver + std::marker::Send,
{
    pub fn new<A>(addrs: Vec<A>) -> Result<Self, Box<dyn std::error::Error>>
    where
        A: std::net::ToSocketAddrs,
    {
        let (tls_config, tls_security_level) =
            match crate::config::get::<String>("tls.security_level")
                .as_ref()
                .map(|c| {
                    <TlsSecurityLevel as std::str::FromStr>::from_str(c)
                        .expect("tls.security_level is not valid")
                })
                .unwrap_or(TlsSecurityLevel::None)
            {
                TlsSecurityLevel::None => (None, TlsSecurityLevel::None),
                level => (Some(Self::get_tls_config()), level),
            };

        Ok(Self {
            listeners: addrs
                .into_iter()
                .filter_map(|addr| std::net::TcpListener::bind(addr).ok())
                .map(|listener| {
                    listener.set_nonblocking(true).unwrap();
                    listener
                })
                .collect::<Vec<_>>(),
            tls_config,
            tls_security_level,
            _phantom: std::marker::PhantomData,
        })
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

    fn get_tls_config() -> std::sync::Arc<rustls::ServerConfig> {
        let capath =
            &crate::config::get::<String>("tls.capath").unwrap_or_else(|_| "./certs".to_string());

        let mut tls_sni_resolver = rustls::server::ResolvesServerCertUsingSni::new();

        crate::config::get::<Vec<SniKey>>("tls.sni_maps")
            .unwrap_or_default()
            .into_iter()
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
            .for_each(|(domain, ck)| tls_sni_resolver.add(&domain, ck).unwrap());

        let mut config = rustls::ServerConfig::builder()
            .with_cipher_suites(rustls::ALL_CIPHER_SUITES)
            .with_kx_groups(&rustls::ALL_KX_GROUPS)
            .with_protocol_versions(rustls::ALL_VERSIONS)
            .expect("inconsistent cipher-suites/versions specified")
            .with_client_cert_verifier(rustls::server::NoClientAuth::new())
            .with_cert_resolver(std::sync::Arc::new(tls_sni_resolver));

        config.ignore_client_order =
            crate::config::get::<bool>("tls.preempt_cipherlist").unwrap_or(false);

        std::sync::Arc::new(config)
    }

    pub fn addr(&self) -> Vec<std::io::Result<std::net::SocketAddr>> {
        self.listeners
            .iter()
            .map(std::net::TcpListener::local_addr)
            .collect::<Vec<_>>()
    }

    fn handle_client(
        &self,
        stream: std::net::TcpStream,
        client_addr: std::net::SocketAddr,
    ) -> Result<(), std::io::Error> {
        log::warn!("Connection from: {}", client_addr);
        let tls_config = self.tls_config.as_ref().map(std::sync::Arc::clone);
        let tls_security_level = self.tls_security_level.clone();

        // ERROR if non blocking == true
        stream.set_nonblocking(true)?;
        // TODO: configurable timeout

        // stream.set_read_timeout(Some(std::time::Duration::from_millis(1)))?;
        // stream.set_write_timeout(Some(std::time::Duration::from_millis(1)))?;

        tokio::spawn(async move {
            log::warn!("Handling client: {}", client_addr);

            match MailReceiver::<R>::new(client_addr, tls_config, tls_security_level)
                .receive_plain(stream)
                .await
            {
                Ok(_) => log::info!("Connection {} closed cleanly", client_addr),
                Err(e) => {
                    log::error!("Connection {} closed with an error {}", client_addr, e)
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
