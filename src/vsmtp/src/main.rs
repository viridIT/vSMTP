/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
 */
use anyhow::Context;
use vsmtp::{Args, Commands};
use vsmtp_common::{
    libc_abstraction::{daemon, initgroups, setgid, setuid},
    re::{anyhow, log, serde_json},
};
use vsmtp_config::{get_log4rs_config, re::log4rs, Config};
use vsmtp_server::start_runtime;

fn socket_bind_anyhow<A: std::net::ToSocketAddrs + std::fmt::Debug>(
    addr: A,
) -> anyhow::Result<std::net::TcpListener> {
    let socket = std::net::TcpListener::bind(&addr)
        .with_context(|| format!("Failed to bind socket on addr: '{:?}'", addr))?;

    socket
        .set_nonblocking(true)
        .with_context(|| format!("Failed to set non-blocking socket on addr: '{:?}'", addr))?;

    Ok(socket)
}

fn main() {
    if let Err(err) = try_main() {
        eprintln!("ERROR: {}", err);
        log::error!("ERROR: {}", err);
        err.chain().skip(1).for_each(|cause| {
            eprintln!("because: {}", cause);
            log::error!("because: {}", cause);
        });
        std::process::exit(1);
    }
}

fn try_main() -> anyhow::Result<()> {
    let args = <Args as clap::StructOpt>::parse();

    let config = args.config.as_ref().map_or_else(
        || Ok(Config::default()),
        |config| {
            std::fs::read_to_string(&config)
                .context(format!("Cannot read file '{}'", config))
                .and_then(|f| Config::from_toml(&f).context("File contains format error"))
                .context("Cannot parse the configuration")
        },
    )?;

    if let Some(command) = args.command {
        match command {
            Commands::ConfigShow => {
                let stringified = serde_json::to_string_pretty(&config)?;
                println!("Loaded configuration: {}", stringified);
                return Ok(());
            }
            Commands::ConfigDiff => {
                let loaded_config = serde_json::to_string_pretty(&config)?;
                let default_config = serde_json::to_string_pretty(&Config::default())?;
                for diff in diff::lines(&default_config, &loaded_config) {
                    match diff {
                        diff::Result::Left(left) => println!("-\x1b[0;31m{left}\x1b[0m"),
                        diff::Result::Both(same, _) => println!(" {same}"),
                        diff::Result::Right(right) => println!("+\x1b[0;32m{right}\x1b[0m"),
                    }
                }
                return Ok(());
            }
        }
    }

    let sockets = (
        socket_bind_anyhow(&config.server.interfaces.addr[..])?,
        socket_bind_anyhow(&config.server.interfaces.addr_submission[..])?,
        socket_bind_anyhow(&config.server.interfaces.addr_submissions[..])?,
    );

    if !args.no_daemon {
        daemon(false, false)?;
        initgroups(
            config.server.system.user.name().to_str().ok_or_else(|| {
                anyhow::anyhow!(
                    "user '{:?}' is not UTF-8 valid",
                    config.server.system.user.name()
                )
            })?,
            config.server.system.group.gid(),
        )?;
        // setresgid ?
        setgid(config.server.system.group.gid())?;
        // setresuid ?
        setuid(config.server.system.user.uid())?;
    }

    get_log4rs_config(&config, args.no_daemon)
        .context("Logs configuration contain error")
        .map(log4rs::init_config)
        .context("Cannot initialize logs")??;

    start_runtime(std::sync::Arc::new(config), sockets).map_err(|e| {
        log::error!("vSMTP terminating error: '{e}'");
        e
    })
}
