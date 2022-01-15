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
    config_deliver: ServerConfig,
    spool_dir: String,
    delivery_receiver: crossbeam_channel::Receiver<String>,
) -> std::io::Result<()> {
    // TODO: check config / rule engine for right resolver.
    // TODO: empty queue when booting.

    async fn handle_one(
        message_id: &str,
        spool_dir: &str,
        config_deliver: &ServerConfig,
    ) -> std::io::Result<()> {
        let deliver_queue = Queue::Deliver.to_path(&spool_dir)?;

        log::debug!(
            target: DELIVER,
            "vDeliver process received a new message id: {}",
            message_id
        );

        let file_to_process = deliver_queue.join(&message_id);
        log::debug!(
            target: DELIVER,
            "vDELIVER opening file: {:?}",
            file_to_process
        );

        let ctx: vsmtp::model::mail::MailContext =
            serde_json::from_str(&std::fs::read_to_string(&file_to_process)?)?;

        log::trace!(target: DELIVER, "content: {:#?}", ctx);

        {
            if let Body::Parsed(content) = &ctx.body {
                log::trace!(target: DELIVER, "raw: {}", {
                    let (headers, body) = content.to_raw();
                    [headers, body].join("\n")
                });
            }
        }

        let mut resolver = MailDirResolver::default();
        resolver.on_data_end(config_deliver, &ctx).await?;

        log::debug!(
            target: DELIVER,
            "message '{}' sent successfully.",
            message_id
        );

        std::fs::remove_file(&file_to_process)?;

        log::debug!(
            target: DELIVER,
            "message '{}' removed from delivery queue.",
            message_id
        );

        Ok(())
    }

    loop {
        if let Ok(message_id) = delivery_receiver.try_recv() {
            // TODO: should not stop process.
            handle_one(&message_id, &spool_dir, &config_deliver)
                .await
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

        let mut ctx: vsmtp::model::mail::MailContext =
            { serde_json::from_str(&std::fs::read_to_string(&file_to_process)?)? };

        ctx.body = Body::Parsed(match ctx.body {
            Body::Parsed(parsed) => {
                log::debug!(target: DELIVER, "parsed email: {:?}", parsed);
                parsed
            }
            Body::Raw(raw) => MailMimeParser::default()
                .parse(raw.as_bytes())
                .map(Box::new)
                .expect("handle errors when parsing email in vMIME"),
        });

        // TODO: run postq rule engine.

        let mut to_deliver = std::fs::OpenOptions::new().create(true).write(true).open(
            std::path::PathBuf::from_iter([
                Queue::Deliver.to_path(&spool_dir)?,
                std::path::Path::new(&message_id).to_path_buf(),
            ]),
        )?;

        std::io::Write::write_all(&mut to_deliver, serde_json::to_string(&ctx)?.as_bytes())?;

        delivery_sender.send(message_id.to_string()).unwrap();

        std::fs::remove_file(&file_to_process)?;

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
        eprintln!("could not initialize the rule engine: {}", error);
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
