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
use crate::config::log::RECEIVER;
use crate::config::server_config::{ServerConfig, TlsSecurityLevel};
use crate::connection::Connection;
use crate::io_service::IoService;
use crate::resolver::DataEndResolver;
use crate::smtp::code::SMTPReplyCode;
use crate::transaction::Transaction;

pub struct ServerVSMTP {
    listener: tokio::net::TcpListener,
    tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    config: std::sync::Arc<ServerConfig>,
}

impl ServerVSMTP {
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
}

impl ServerVSMTP {
    pub async fn listen_and_serve<R>(
        &self,
        resolver: std::sync::Arc<tokio::sync::Mutex<R>>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        R: DataEndResolver + std::marker::Send + std::marker::Sync + 'static,
    {
        loop {
            match self.listener.accept().await {
                Ok((stream, client_addr)) => {
                    log::warn!("Connection from: {}", client_addr);

                    let mut stream = stream.into_std()?;
                    stream.set_nonblocking(true)?;

                    let config = self.config.clone();
                    let tls_config = self.tls_config.as_ref().map(std::sync::Arc::clone);

                    let resolver = resolver.clone();

                    tokio::spawn(async move {
                        let begin = std::time::SystemTime::now();
                        log::warn!("Handling client: {}", client_addr);

                        let mut io_plain = IoService::new(&mut stream);

                        let mut conn = Connection::<std::net::TcpStream>::from_plain(
                            client_addr,
                            config.clone(),
                            &mut io_plain,
                        )?;
                        Self::handle_connection::<R, std::net::TcpStream>(
                            &mut conn, resolver, tls_config,
                        )
                        .await
                        .map(|_| {
                            log::warn!(
                                "{{ elapsed: {:?} }} Connection {} closed cleanly",
                                begin.elapsed(),
                                client_addr,
                            );
                        })
                        .map_err(|error| {
                            log::error!(
                                "{{ elapsed: {:?} }} Connection {} closed with an error {}",
                                begin.elapsed(),
                                client_addr,
                                error,
                            );
                            error
                        })
                    });
                }
                Err(e) => log::error!("Error accepting socket; error = {:?}", e),
            }
        }
    }

    pub async fn handle_connection<R, S>(
        conn: &mut Connection<'_, S>,
        resolver: std::sync::Arc<tokio::sync::Mutex<R>>,
        tls_config: Option<std::sync::Arc<rustls::ServerConfig>>,
    ) -> Result<(), std::io::Error>
    where
        R: crate::resolver::DataEndResolver,
        S: std::io::Read + std::io::Write,
    {
        let mut helo_domain = None;

        conn.send_code(SMTPReplyCode::Code220)?;

        while conn.is_alive {
            match Transaction::receive(conn, &helo_domain).await? {
                crate::transaction::TransactionResult::Nothing => {}
                crate::transaction::TransactionResult::Mail(mail) => {
                    helo_domain = Some(mail.envelop.helo.clone());

                    // writing the context to the delivery queue.
                    let code = resolver
                        .lock()
                        .await
                        .on_data_end(&conn.config, &mail)
                        .await?;

                    conn.send_code(code)?;
                }
                crate::transaction::TransactionResult::TlsUpgrade if tls_config.is_none() => {
                    conn.send_code(SMTPReplyCode::Code454)?;
                    conn.send_code(SMTPReplyCode::Code221)?;
                    return Ok(());
                }
                crate::transaction::TransactionResult::TlsUpgrade => {
                    let mut tls_conn = rustls::ServerConnection::new(tls_config.unwrap()).unwrap();
                    let mut tls_stream = rustls::Stream::new(&mut tls_conn, &mut conn.io_stream);
                    let mut io_tls_stream = IoService::new(&mut tls_stream);

                    Connection::<IoService<'_, S>>::complete_tls_handshake(
                        &mut io_tls_stream,
                        &conn.config.tls.handshake_timeout,
                    )?;

                    let mut secured_conn = Connection {
                        timestamp: conn.timestamp,
                        is_alive: true,
                        config: conn.config.clone(),
                        client_addr: conn.client_addr,
                        error_count: conn.error_count,
                        is_secured: true,
                        io_stream: &mut io_tls_stream,
                    };

                    let conn = &secured_conn.io_stream.inner.conn;

                    log::debug!(
                        target: RECEIVER,
                        "protocol_version={:#?}",
                        conn.protocol_version()
                    );
                    log::debug!(
                        target: RECEIVER,
                        "alpn_protocol={:#?}",
                        conn.alpn_protocol()
                    );
                    log::debug!(
                        target: RECEIVER,
                        "negotiated_cipher_suite={:#?}",
                        conn.negotiated_cipher_suite()
                    );
                    log::debug!(
                        target: RECEIVER,
                        "peer_certificates={:#?}",
                        conn.peer_certificates()
                    );
                    log::debug!(target: RECEIVER, "sni_hostname={:#?}", conn.sni_hostname());

                    let mut secured_helo_domain = None;

                    while secured_conn.is_alive {
                        match Transaction::receive(&mut secured_conn, &secured_helo_domain).await? {
                            crate::transaction::TransactionResult::Nothing => {}
                            crate::transaction::TransactionResult::Mail(mail) => {
                                secured_helo_domain = Some(mail.envelop.helo.clone());

                                let code = resolver
                                    .lock()
                                    .await
                                    .on_data_end(&secured_conn.config, &mail)
                                    .await?;

                                secured_conn.send_code(code)?;
                            }
                            crate::transaction::TransactionResult::TlsUpgrade => todo!(),
                        }
                    }
                    return Ok(());
                }
            }
        }

        Ok(())
    }
}

/// indentifiers for all mail queues.
#[allow(unused)]
enum Queue {
    Deliver,
    Working,
    Deferred,
    Dead,
}

impl Queue {
    fn as_str(&self) -> &'static str {
        match self {
            Queue::Deliver => "deliver",
            Queue::Working => "working",
            Queue::Deferred => "deferred",
            Queue::Dead => "dead",
        }
    }
}

/// used to write mail to the delivery queue and send a notification
/// to the delivery process.
pub struct DeliverQueueResolver {
    deliver_proc: crossbeam_channel::Sender<String>,
}

impl DeliverQueueResolver {
    pub fn new(sender: crossbeam_channel::Sender<String>) -> Self {
        Self {
            deliver_proc: sender,
        }
    }
}

#[async_trait::async_trait]
impl DataEndResolver for DeliverQueueResolver {
    async fn on_data_end(
        &mut self,
        server_config: &ServerConfig,
        ctx: &crate::model::mail::MailContext,
    ) -> Result<SMTPReplyCode, std::io::Error> {
        write_to_queue(Queue::Deliver, server_config, ctx)?;

        let message_id = ctx.metadata.as_ref().unwrap().message_id.clone();

        log::trace!(
            target: RECEIVER,
            "mail {} successfuly written to deliver queue",
            message_id
        );

        // TODO: handle send errors.
        // sending the message id to the delivery process.
        // NOTE: we could send the context instead, so that the delivery system won't have
        //       to touch the file system.
        self.deliver_proc.send(message_id).unwrap();

        // TODO: use the right codes.
        Ok(SMTPReplyCode::Code250)
    }
}

/// write a mail as JSON to the given queue using it's message id.
fn write_to_queue(
    queue: Queue,
    server_config: &ServerConfig,
    ctx: &crate::model::mail::MailContext,
) -> std::io::Result<()> {
    let message_id = ctx.metadata.as_ref().unwrap().message_id.clone();

    let to_deliver = <std::path::PathBuf as std::str::FromStr>::from_str(&format!(
        "{}/{}/{}",
        server_config.smtp.spool_dir,
        queue.as_str(),
        &message_id
    ))
    // unfailable.
    .unwrap();

    // TODO: should loop if a file name is conflicting.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(&to_deliver)?;

    std::io::Write::write_all(&mut file, serde_json::to_string(ctx)?.as_bytes())
}
