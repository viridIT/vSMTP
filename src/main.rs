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
use vsmtp::config::get_logger_config;
use vsmtp::config::log_channel::DELIVER;
use vsmtp::config::server_config::ServerConfig;
use vsmtp::mime::parser::MailMimeParser;
use vsmtp::model::mail::Body;
use vsmtp::resolver::deliver_queue::{DeliverQueueResolver, Queue};
use vsmtp::resolver::smtp_resolver::SMTPResolver;
use vsmtp::resolver::DataEndResolver;
use vsmtp::rules::rule_engine;
use vsmtp::server::ServerVSMTP;

#[derive(clap::Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(short, long)]
    config: String,
}

async fn v_deliver(
    config: std::sync::Arc<ServerConfig>,
    mut delivery_receiver: tokio::sync::mpsc::Receiver<String>,
) -> std::io::Result<()> {
    async fn handle_one_in_deferred_queue(
        path: &std::path::Path,
        config: &ServerConfig,
    ) -> std::io::Result<()> {
        let message_id = path.file_name().and_then(|i| i.to_str()).unwrap();

        log::debug!(
            target: DELIVER,
            "vDeliver (deferred) RECEIVED '{}'",
            message_id
        );

        let mut file = std::fs::OpenOptions::new().read(true).open(&path)?;

        let mut raw = String::with_capacity(file.metadata().unwrap().len() as usize);
        std::io::Read::read_to_string(&mut file, &mut raw)?;

        let mut mail: vsmtp::model::mail::MailContext = serde_json::from_str(&raw)?;
        assert_eq!(message_id, mail.metadata.as_ref().unwrap().message_id);

        let max_retry_deferred = config
            .delivery
            .queue
            .get("deferred")
            .map(|q| q.retry_max)
            .flatten()
            .unwrap_or(100);

        if mail.metadata.as_ref().unwrap().retry >= max_retry_deferred {
            log::warn!(
                target: DELIVER,
                "vDeliver (deferred) '{}' MAX RETRY, retry: '{}'",
                message_id,
                mail.metadata.as_ref().unwrap().retry
            );

            std::fs::rename(
                path,
                std::path::PathBuf::from_iter([
                    Queue::Dead.to_path(&config.smtp.spool_dir)?,
                    std::path::Path::new(&message_id).to_path_buf(),
                ]),
            )?;

            log::info!(
                target: DELIVER,
                "vDeliver (deferred) '{}' MOVED deferred => dead.",
                message_id
            );
        } else {
            let mut resolver = SMTPResolver::default();
            match resolver.on_data_end(config, &mail).await {
                Ok(_) => {
                    log::trace!(
                        target: DELIVER,
                        "vDeliver (deferred) '{}' SEND successfully.",
                        message_id
                    );

                    std::fs::remove_file(&path)?;

                    log::info!(
                        target: DELIVER,
                        "vDeliver (deferred) '{}' REMOVED successfully.",
                        message_id
                    );
                }
                Err(error) => {
                    log::warn!(
                        target: DELIVER,
                        "vDeliver (deferred) '{}' SEND FAILED, reason: '{}'",
                        message_id,
                        error
                    );

                    mail.metadata.as_mut().unwrap().retry += 1;

                    let mut file = std::fs::OpenOptions::new()
                        .truncate(true)
                        .write(true)
                        .open(&path)
                        .unwrap();

                    std::io::Write::write_all(&mut file, serde_json::to_string(&mail)?.as_bytes())?;

                    log::info!(
                        target: DELIVER,
                        "vDeliver (deferred) '{}' INCREASE retries to '{}'.",
                        message_id,
                        mail.metadata.as_mut().unwrap().retry
                    );
                }
            }
        }

        Ok(())
    }

    async fn flush_deferred_queue(config: &ServerConfig) -> std::io::Result<()> {
        for path in std::fs::read_dir(Queue::Deferred.to_path(&config.smtp.spool_dir)?)? {
            handle_one_in_deferred_queue(&path?.path(), config)
                .await
                .unwrap();
        }

        Ok(())
    }

    log::info!(
        target: DELIVER,
        "vDeliver (deferred) booting, flushing queue.",
    );
    flush_deferred_queue(&config).await?;

    async fn handle_one_in_delivery_queue(
        path: &std::path::Path,
        config: &ServerConfig,
    ) -> std::io::Result<()> {
        let message_id = path.file_name().and_then(|i| i.to_str()).unwrap();

        log::trace!(
            target: DELIVER,
            "vDeliver (delivery) RECEIVED '{}'",
            message_id
        );

        let mut file = std::fs::OpenOptions::new().read(true).open(&path)?;

        let mut raw = String::with_capacity(file.metadata().unwrap().len() as usize);
        std::io::Read::read_to_string(&mut file, &mut raw)?;

        let mail: vsmtp::model::mail::MailContext = serde_json::from_str(&raw)?;
        assert_eq!(message_id, mail.metadata.as_ref().unwrap().message_id);

        let mut resolver = SMTPResolver::default();
        match resolver.on_data_end(config, &mail).await {
            Ok(_) => {
                log::trace!(
                    target: DELIVER,
                    "vDeliver (delivery) '{}' SEND successfully.",
                    message_id
                );

                std::fs::remove_file(&path)?;

                log::info!(
                    target: DELIVER,
                    "vDeliver (delivery) '{}' REMOVED successfully.",
                    message_id
                );
            }
            Err(error) => {
                log::warn!(
                    target: DELIVER,
                    "vDeliver (delivery) '{}' SEND FAILED, reason: '{}'",
                    message_id,
                    error
                );

                std::fs::rename(
                    path,
                    std::path::PathBuf::from_iter([
                        Queue::Deferred.to_path(&config.smtp.spool_dir)?,
                        std::path::Path::new(&message_id).to_path_buf(),
                    ]),
                )?;

                log::info!(
                    target: DELIVER,
                    "vDeliver (delivery) '{}' MOVED delivery => deferred.",
                    message_id
                );
            }
        }

        Ok(())
    }

    async fn flush_deliver_queue(config: &ServerConfig) -> std::io::Result<()> {
        for path in std::fs::read_dir(Queue::Deliver.to_path(&config.smtp.spool_dir)?)? {
            handle_one_in_delivery_queue(&path?.path(), config)
                .await
                .unwrap();
        }

        Ok(())
    }

    log::info!(
        target: DELIVER,
        "vDeliver (delivery) booting, flushing queue.",
    );
    flush_deliver_queue(&config).await?;

    let mut flush_deferred_interval = tokio::time::interval(
        config
            .delivery
            .queue
            .get("deferred")
            .map(|q| q.cron_period)
            .flatten()
            .unwrap_or_else(|| std::time::Duration::from_secs(10)),
    );

    loop {
        tokio::select! {
            Some(message_id) = delivery_receiver.recv() => {
                handle_one_in_delivery_queue(
                    &std::path::PathBuf::from_iter([
                        Queue::Deliver.to_path(&config.smtp.spool_dir)?,
                        std::path::Path::new(&message_id).to_path_buf(),
                    ]),
                    &config,
                )
                .await
                .unwrap();
            }
            _ = flush_deferred_interval.tick() => {
                log::info!(
                    target: DELIVER,
                    "vDeliver (deferred) cronjob delay elapsed, flushing queue.",
                );
                flush_deferred_queue(&config).await.unwrap();
            }
        };
    }
}

async fn v_mime(
    spool_dir: String,
    mut working_receiver: tokio::sync::mpsc::Receiver<String>,
    delivery_sender: tokio::sync::mpsc::Sender<String>,
) -> std::io::Result<()> {
    async fn handle_one(
        message_id: &str,
        spool_dir: &str,
        delivery_sender: &tokio::sync::mpsc::Sender<String>,
    ) -> std::io::Result<()> {
        log::debug!(
            target: DELIVER,
            "vMIME process received a new message id: {}",
            message_id
        );

        let working_queue = Queue::Working.to_path(spool_dir)?;

        let file_to_process = working_queue.join(&message_id);
        log::debug!(target: DELIVER, "vMIME opening file: {:?}", file_to_process);

        let mail: vsmtp::model::mail::MailContext =
            serde_json::from_str(&std::fs::read_to_string(&file_to_process)?)?;

        match mail.body {
            Body::Parsed(_) => {}
            Body::Raw(ref raw) => {
                MailMimeParser::default()
                    .parse(raw.as_bytes())
                    // .and_then(|_| todo!("run postq rule engine"))
                    .expect("handle errors when parsing email in vMIME");
            }
        };

        // TODO: run postq rule engine.

        let mut to_deliver = std::fs::OpenOptions::new().create(true).write(true).open(
            std::path::PathBuf::from_iter([
                Queue::Deliver.to_path(&spool_dir)?,
                std::path::Path::new(&message_id).to_path_buf(),
            ]),
        )?;

        std::io::Write::write_all(&mut to_deliver, serde_json::to_string(&mail)?.as_bytes())?;

        delivery_sender.send(message_id.to_string()).await.unwrap();

        std::fs::remove_file(&file_to_process)?;

        log::debug!(
            target: DELIVER,
            "message '{}' removed from working queue.",
            message_id
        );

        Ok(())
    }

    loop {
        if let Some(message_id) = working_receiver.recv().await {
            handle_one(&message_id, &spool_dir, &delivery_sender)
                .await
                .unwrap();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = <Args as clap::StructOpt>::parse();
    println!("Loading with configuration: '{}'", args.config);

    let mut config: ServerConfig =
        toml::from_str(&std::fs::read_to_string(args.config).expect("cannot read file"))
            .expect("cannot parse config from toml");
    config.prepare();
    let config = std::sync::Arc::new(config);

    log4rs::init_config(get_logger_config(&config)?)?;

    // TODO: move into server init.
    // creating the spool folder if it doesn't exists yet.
    {
        let spool_dir =
            <std::path::PathBuf as std::str::FromStr>::from_str(&config.smtp.spool_dir).unwrap();

        if !spool_dir.exists() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(spool_dir)?;
        }
    }

    rule_engine::init(Box::leak(config.rules.dir.clone().into_boxed_str())).map_err(|error| {
        log::error!("could not initialize the rule engine: {}", error);
        error
    })?;

    let delivery_buffer_size = config
        .delivery
        .queue
        .get("delivery")
        .map(|q| q.capacity)
        .flatten()
        .unwrap_or(1);

    let (delivery_sender, delivery_receiver) =
        tokio::sync::mpsc::channel::<String>(delivery_buffer_size);

    let working_buffer_size = config
        .delivery
        .queue
        .get("working")
        .map(|q| q.capacity)
        .flatten()
        .unwrap_or(1);

    let (working_sender, working_receiver) =
        tokio::sync::mpsc::channel::<String>(working_buffer_size);

    let config_deliver = config.clone();
    tokio::spawn(async move {
        let result = v_deliver(config_deliver, delivery_receiver).await;
        log::error!("v_deliver ended unexpectedly '{:?}'", result);
    });

    let config_mime = config.clone();
    tokio::spawn(async move {
        let result = v_mime(
            config_mime.smtp.spool_dir.clone(),
            working_receiver,
            delivery_sender,
        )
        .await;
        log::error!("v_mime ended unexpectedly '{:?}'", result);
    });

    let server = ServerVSMTP::new(config)
        .await
        .expect("Failed to create the server");
    log::warn!("Listening on: {:?}", server.addr());

    server
        .listen_and_serve(std::sync::Arc::new(tokio::sync::Mutex::new(
            DeliverQueueResolver::new(working_sender),
        )))
        .await
}
