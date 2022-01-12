use vsmtp::config::get_logger_config;
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
use vsmtp::config::log_channel::DELIVER;
use vsmtp::config::server_config::ServerConfig;
use vsmtp::mime::parser::MailMimeParser;
use vsmtp::model::mail::Body;
use vsmtp::resolver::deliver_queue::{DeliverQueueResolver, Queue};
use vsmtp::resolver::maildir_resolver::MailDirResolver;
use vsmtp::resolver::DataEndResolver;
use vsmtp::rules::rule_engine;

#[derive(clap::Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(short, long)]
    config: String,
}

async fn v_deliver(
    config: ServerConfig,
    spool_dir: String,
    delivery_receiver: crossbeam_channel::Receiver<String>,
) -> std::io::Result<()> {
    // TODO: check config / rule engine for right resolver.

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

        // TODO: if retry >= max_retry...

        let mut resolver = MailDirResolver::default();
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

        Ok(())
    }

    async fn flush_deferred_queue(spool_dir: &str, config: &ServerConfig) -> std::io::Result<()> {
        for path in std::fs::read_dir(Queue::Deferred.to_path(&spool_dir)?)? {
            handle_one_in_deferred_queue(&path?.path(), config)
                .await
                .unwrap();
        }

        Ok(())
    }

    flush_deferred_queue(&spool_dir, &config).await?;

    async fn handle_one_in_delivery_queue(
        path: &std::path::Path,
        spool_dir: &str,
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

        let mut resolver = MailDirResolver::default();
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
                        Queue::Deferred.to_path(&spool_dir)?,
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

    async fn flush_deliver_queue(spool_dir: &str, config: &ServerConfig) -> std::io::Result<()> {
        for path in std::fs::read_dir(Queue::Deliver.to_path(&spool_dir)?)? {
            handle_one_in_delivery_queue(&path?.path(), spool_dir, config)
                .await
                .unwrap();
        }

        Ok(())
    }

    flush_deliver_queue(&spool_dir, &config).await?;

    loop {
        if let Ok(message_id) = delivery_receiver.try_recv() {
            handle_one_in_delivery_queue(
                &std::path::PathBuf::from_iter([
                    Queue::Deliver.to_path(&spool_dir)?,
                    std::path::Path::new(&message_id).to_path_buf(),
                ]),
                &spool_dir,
                &config,
            )
            .await
            // TODO: should not stop process.
            .unwrap();
        }
    }
}

async fn v_mime(
    spool_dir: String,
    working_receiver: crossbeam_channel::Receiver<String>,
    delivery_sender: crossbeam_channel::Sender<String>,
) -> std::io::Result<()> {
    fn handle_one(
        message_id: &str,
        spool_dir: &str,
        delivery_sender: &crossbeam_channel::Sender<String>,
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

        let _ = match mail.body {
            Body::Parsed(_) => unreachable!(),
            Body::Raw(raw) => MailMimeParser::default()
                .parse(raw.as_bytes())
                // .and_then(|_| todo!("run postq rule engine"))
                .expect("handle errors when parsing email in vMIME"),
        };

        delivery_sender.send(message_id.to_string()).unwrap();

        std::fs::rename(
            file_to_process,
            std::path::PathBuf::from_iter([
                Queue::Deliver.to_path(&spool_dir)?,
                std::path::Path::new(&message_id).to_path_buf(),
            ]),
        )?;

        log::debug!(
            target: DELIVER,
            "message '{}' removed from working queue.",
            message_id
        );

        Ok(())
    }

    loop {
        if let Ok(message_id) = working_receiver.try_recv() {
            handle_one(&message_id, &spool_dir, &delivery_sender).unwrap();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = <Args as clap::StructOpt>::parse();
    println!("Loading with configuration: '{}'", args.config);

    let config: ServerConfig =
        toml::from_str(&std::fs::read_to_string(args.config).expect("cannot read file"))
            .expect("cannot parse config from toml");

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

    let (delivery_sender, delivery_receiver) = crossbeam_channel::bounded::<String>(0);
    let (working_sender, working_receiver) = crossbeam_channel::bounded::<String>(0);

    tokio::spawn(v_deliver(
        config.clone(),
        config.smtp.spool_dir.clone(),
        delivery_receiver,
    ));

    tokio::spawn(v_mime(
        config.smtp.spool_dir.clone(),
        working_receiver,
        delivery_sender,
    ));

    let server = config.build().await;
    log::warn!("Listening on: {:?}", server.addr());

    server
        .listen_and_serve(std::sync::Arc::new(tokio::sync::Mutex::new(
            DeliverQueueResolver::new(working_sender),
        )))
        .await
}
