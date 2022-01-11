use vsmtp::config::log::DELIVER;
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
use vsmtp::config::server_config::ServerConfig;
use vsmtp::resolver::maildir_resolver::MailDirResolver;
use vsmtp::resolver::DataEndResolver;
use vsmtp::rules::rule_engine;

#[derive(clap::Parser, Debug)]
#[clap(about, version, author)]
struct Args {
    #[clap(short, long)]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = <Args as clap::StructOpt>::parse();

    log::warn!("Loading with configuration: \"{:?}\"", args.config);

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

    // the leak is needed to pass from &'a str to &'static str
    // and initialize the rule engine's rule directory.
    let rules_dir = config.rules.dir.clone();
    if let Err(error) = rule_engine::init(Box::leak(rules_dir.into_boxed_str())) {
        // we can't use logs here because it is initialized when building the server.
        // NOTE: should we remove the log initialization inside the server ?
        eprintln!("could not initialize the rule engine: {}", error);
        return Err(error);
    }

    let (s, r) = crossbeam_channel::bounded::<String>(0);

    let mut deliver_queue = <std::path::PathBuf as std::str::FromStr>::from_str(&format!(
        "{}/deliver/",
        config.smtp.spool_dir
    ))
    // unfailable.
    .unwrap();

    if !deliver_queue.exists() {
        std::fs::DirBuilder::new().create(&deliver_queue)?;
    }

    let config_deliver = config.clone();

    // vDeliver process.
    tokio::spawn(async move {
        // TODO: check config / rule engine for right resolver.
        // TODO: empty queue when booting.
        let mut resolver = MailDirResolver::default();
        deliver_queue.push("tmp");

        loop {
            if let Ok(message_id) = r.try_recv() {
                log::debug!(
                    target: DELIVER,
                    "delivery process received a new message id: {}",
                    message_id
                );

                // open the file.
                let mail: vsmtp::model::mail::MailContext = {
                    deliver_queue.set_file_name(&message_id);
                    // TODO: should not stop process.
                    serde_json::from_str(&std::fs::read_to_string(&deliver_queue)?)?
                };

                // send mail.
                resolver.on_data_end(&config_deliver, &mail).await?;

                log::debug!(
                    target: DELIVER,
                    "message '{}' sent successfuly.",
                    message_id
                );

                // remove mail from queue.
                // TODO: should not stop process.
                std::fs::remove_file(&deliver_queue)?;

                log::debug!(
                    target: DELIVER,
                    "message '{}' removed from delivery queue.",
                    message_id
                );

                return std::io::Result::Ok(());
            }
        }
    });

    let server = config.build().await;
    log::warn!("Listening on: {:?}", server.addr());

    server
        .listen_and_serve(std::sync::Arc::new(tokio::sync::Mutex::new(
            vsmtp::server::DeliverQueueResolver::new(s),
        )))
        .await
}
